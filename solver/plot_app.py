"""
A small window for drawing stored solutions.

    python -m solver.plot_app

Left, the instances with their best length (type in the box to filter).
Middle, the stored runs of the instance clicked last, shortest first. Click
selects one, shift click a range, ctrl click adds one, ctrl a selects all.

- Draw selected runs: a picture of every selected run.
- Draw bests: a picture of the best of every selected instance.

Pictures go to `plots/` the same way `solver.plot` writes them, and the last
one shows on the right. Double click the preview to open it full size. A
run whose picture already exists shows it when clicked.
"""

import csv
import os
import queue
import threading
import tkinter as tk
from pathlib import Path
from tkinter import ttk

from PIL import Image, ImageTk

from cgshop2027_pyutils.instance_database import InstanceDatabase
from cgshop2027_pyutils.io import read_solution

from .mine import DEFAULT_INSTANCES
from .plot import BEST, CANDIDATES, DEFAULT_OUT, SUFFIX, Target, draw, run_name, runs_of

LEADERBOARD = Path("data") / "leaderboard.csv"

# how often the window looks for news from the drawing thread, in ms
POLL_MS = 100

PREVIEW_SIZE = 640


class PlotApp:
    def __init__(self, root: tk.Tk) -> None:
        self.root = root
        self.root.title("CG:SHOP 2027 solution pictures")
        self.root.geometry("1400x760")

        self.instances = _read_instances()
        self.shown_instances: list[str] = []
        self.shown_runs: list[Target] = []
        self.current_instance: str | None = None

        self.messages: queue.Queue = queue.Queue()
        self.cancel = threading.Event()
        self.worker: threading.Thread | None = None
        self.preview_path: Path | None = None
        self.preview_image = None

        self._build()
        self._fill_instances()
        self.root.after(POLL_MS, self._poll)

    # layout

    def _build(self) -> None:
        self.root.columnconfigure(2, weight=1)
        self.root.rowconfigure(1, weight=1)

        ttk.Label(self.root, text="Instances").grid(row=0, column=0, sticky="w", padx=8, pady=(8, 0))
        ttk.Label(self.root, text="Runs of the instance clicked last").grid(
            row=0, column=1, sticky="w", padx=8, pady=(8, 0)
        )
        ttk.Label(self.root, text="Preview (double click to open)").grid(
            row=0, column=2, sticky="w", padx=8, pady=(8, 0)
        )

        # instances, with a filter box on top
        left = ttk.Frame(self.root)
        left.grid(row=1, column=0, sticky="nsew", padx=8, pady=4)
        left.rowconfigure(1, weight=1)

        self.filter_text = tk.StringVar()
        self.filter_text.trace_add("write", lambda *_: self._fill_instances())
        ttk.Entry(left, textvariable=self.filter_text).grid(row=0, column=0, columnspan=2, sticky="ew")

        self.instance_list = _listbox(left, width=44)
        self.instance_list.bind("<<ListboxSelect>>", self._on_instance)

        # runs of one instance
        middle = ttk.Frame(self.root)
        middle.grid(row=1, column=1, sticky="nsew", padx=8, pady=4)
        middle.rowconfigure(1, weight=1)

        self.run_count = ttk.Label(middle, text="")
        self.run_count.grid(row=0, column=0, sticky="w")

        self.run_list = _listbox(middle, width=58)
        self.run_list.bind("<<ListboxSelect>>", self._on_run)

        # preview
        self.preview = ttk.Label(self.root, anchor="center")
        self.preview.grid(row=1, column=2, sticky="nsew", padx=8, pady=4)
        self.preview.bind("<Double-Button-1>", self._open_preview)

        # options and buttons
        bottom = ttk.Frame(self.root)
        bottom.grid(row=2, column=0, columnspan=3, sticky="ew", padx=8, pady=8)

        ttk.Button(bottom, text="Draw selected runs", command=self._draw_runs).pack(side="left")
        ttk.Button(bottom, text="Draw bests of selected instances", command=self._draw_bests).pack(
            side="left", padx=6
        )
        ttk.Button(bottom, text="Stop", command=self.cancel.set).pack(side="left")
        ttk.Button(bottom, text="Open plots folder", command=self._open_folder).pack(side="left", padx=6)

        self.bare = tk.BooleanVar(value=False)
        self.gif = tk.BooleanVar(value=False)
        self.dpi = tk.IntVar(value=200)
        ttk.Checkbutton(bottom, text="bare", variable=self.bare).pack(side="left", padx=(16, 0))
        ttk.Checkbutton(bottom, text="gif", variable=self.gif).pack(side="left", padx=6)
        ttk.Label(bottom, text="dpi").pack(side="left")
        ttk.Spinbox(bottom, from_=100, to=600, increment=100, width=5, textvariable=self.dpi).pack(side="left")

        self.status = ttk.Label(bottom, text="ready")
        self.status.pack(side="left", padx=16)

    # lists

    def _fill_instances(self) -> None:
        text = self.filter_text.get().strip() if hasattr(self, "filter_text") else ""

        self.shown_instances = [uid for uid in self.instances if text in uid]
        self.instance_list.delete(0, "end")

        for uid in self.shown_instances:
            best = self.instances[uid]
            self.instance_list.insert("end", f"{uid:34} {best:>8}")

    def _on_instance(self, _event) -> None:
        picked = self.instance_list.curselection()
        if not picked:
            return

        # the runs follow the instance clicked last
        uid = self.shown_instances[picked[-1]]
        if uid == self.current_instance:
            return

        self.current_instance = uid
        self._fill_runs(uid)

    def _fill_runs(self, uid: str) -> None:
        rows = []
        for path in runs_of(uid, CANDIDATES):
            objective = read_solution(path).objective
            name = run_name(uid, path)
            rows.append((objective, name, Target(path, DEFAULT_OUT / uid / f"{name}.png")))

        rows.sort(key=lambda row: row[0])

        self.shown_runs = [target for _, _, target in rows]
        self.run_list.delete(0, "end")
        for objective, name, _ in rows:
            self.run_list.insert("end", f"{objective:>8}  {name}")

        self.run_count.config(text=f"{uid}: {len(rows)} stored runs")

    def _on_run(self, _event) -> None:
        picked = self.run_list.curselection()
        if len(picked) != 1:
            return

        picture = self.shown_runs[picked[0]].picture
        if picture.exists():
            self._show(picture)

    # drawing, in a thread so the window stays responsive

    def _draw_runs(self) -> None:
        targets = [self.shown_runs[index] for index in self.run_list.curselection()]
        self._start(targets)

    def _draw_bests(self) -> None:
        targets = []
        for index in self.instance_list.curselection():
            uid = self.shown_instances[index]
            targets.append(Target(BEST / f"{uid}{SUFFIX}", DEFAULT_OUT / f"{uid}.png"))

        self._start(targets)

    def _start(self, targets: list[Target]) -> None:
        if not targets:
            self.status.config(text="select something first")
            return

        if self.worker is not None and self.worker.is_alive():
            self.status.config(text="still drawing, press Stop first")
            return

        self.cancel.clear()
        options = {"bare": self.bare.get(), "gif": self.gif.get(), "dpi": int(self.dpi.get())}

        self.worker = threading.Thread(target=self._work, args=(targets, options), daemon=True)
        self.worker.start()

    def _work(self, targets: list[Target], options: dict) -> None:
        database = InstanceDatabase(str(DEFAULT_INSTANCES))
        total = len(targets)
        done = 0

        for number, target in enumerate(targets, start=1):
            if self.cancel.is_set():
                self.messages.put(("status", f"stopped, {done} of {total} drawn"))
                return

            self.messages.put(("status", f"drawing {number} of {total}: {target.solution.name}"))

            # one broken picture should not stop the rest
            try:
                draw(target, database, **options)
            except Exception as error:
                self.messages.put(("status", f"failed {target.solution.name}: {error}"))
                continue

            done += 1
            self.messages.put(("picture", target.picture))

        self.messages.put(("status", f"done, {done} of {total} drawn into {DEFAULT_OUT}"))

    def _poll(self) -> None:
        while not self.messages.empty():
            kind, value = self.messages.get()
            if kind == "status":
                self.status.config(text=value)
            elif kind == "picture":
                self._show(value)

        self.root.after(POLL_MS, self._poll)

    # preview

    def _show(self, picture: Path) -> None:
        with Image.open(picture) as image:
            image.thumbnail((PREVIEW_SIZE, PREVIEW_SIZE))
            self.preview_image = ImageTk.PhotoImage(image)

        self.preview.config(image=self.preview_image)
        self.preview_path = picture

    def _open_preview(self, _event) -> None:
        if self.preview_path is not None:
            os.startfile(self.preview_path.resolve())

    def _open_folder(self) -> None:
        DEFAULT_OUT.mkdir(exist_ok=True)
        os.startfile(DEFAULT_OUT.resolve())


def _listbox(parent: ttk.Frame, *, width: int) -> tk.Listbox:
    # extended selection: click, shift click for a range, ctrl click to add
    listbox = tk.Listbox(
        parent, selectmode="extended", width=width, exportselection=False, font=("Consolas", 10)
    )
    listbox.grid(row=1, column=0, sticky="nsew")

    scroll = ttk.Scrollbar(parent, orient="vertical", command=listbox.yview)
    scroll.grid(row=1, column=1, sticky="ns")
    listbox.config(yscrollcommand=scroll.set)

    listbox.bind("<Control-a>", lambda _: listbox.select_set(0, "end"))

    return listbox


def _read_instances() -> dict[str, int]:
    """
    Every instance with a stored best and its length, by name.
    """
    instances = {}
    with LEADERBOARD.open() as file:
        for row in csv.DictReader(file):
            if row["best"] and (BEST / f"{row['instance']}{SUFFIX}").exists():
                instances[row["instance"]] = int(row["best"])

    return dict(sorted(instances.items()))


def main() -> None:
    root = tk.Tk()
    PlotApp(root)
    root.mainloop()


if __name__ == "__main__":
    main()
