import matplotlib

matplotlib.use("Agg")

from pathlib import Path

import matplotlib.pyplot as plt

from cgshop2027_pyutils.instance_database import InstanceDatabase
from cgshop2027_pyutils.visualize import create_solution_plot
from cgshop2027_pyutils.zip import ZipSolutionIterator

out = Path("plots")
out.mkdir(exist_ok=True)

db = InstanceDatabase("example_instances.zip")
for solution in ZipSolutionIterator("example_solutions.zip"):
    instance = db[solution.instance_uid]
    fig = create_solution_plot(instance, solution)
    fig.savefig(out / f"{solution.instance_uid}.png")
    plt.close(fig)
    print("saved", solution.instance_uid, "max tour length:", solution.max_tour_length)