"""
Admission control for the miner: a job starts only when the machine can
hold it.

There are two counting semaphores, one per resource:

- cores, one permit per core
- memory, one permit per megabyte

Both are sized from the machine when the miner starts. Every job asks for a
`Demand` of both, and its worker process is spawned only once the scheduler
has taken all of it. When the process ends, the permits go back. So the
running jobs never ask for more than the machine has, however many there are.

The scheduler is one thread that never waits on a permit, so acquiring is a
non blocking try that takes all of a demand or none of it.
"""

import ctypes
import os
import sys
from dataclasses import dataclass

# psutil is optional, the fallbacks below ask the system directly
try:
    import psutil
except ImportError:
    psutil = None

__all__ = [
    "CountingSemaphore",
    "Demand",
    "Resources",
    "available_memory_mb",
    "system_resources",
]

# leave this much memory to the rest of the machine
MEMORY_RESERVE_MB = 2048

# and plan with only this share of what is left, since others may grow
MEMORY_SHARE = 0.8

# one core stays free for the scheduler and the desktop
CORES_RESERVED = 1


class CountingSemaphore:
    """
    A pool of `capacity` permits. Owned by the scheduler thread alone.
    """

    def __init__(self, name: str, capacity: int) -> None:
        if capacity < 1:
            message = f"{name} needs at least one permit, got {capacity}"
            raise ValueError(message)

        self.name = name
        self.capacity = capacity
        self.free = capacity

    def try_acquire(self, permits: int) -> bool:
        if permits > self.capacity:
            message = f"{permits} {self.name} permits asked, only {self.capacity} exist"
            raise ValueError(message)

        if permits > self.free:
            return False

        self.free -= permits
        return True

    def release(self, permits: int) -> None:
        if self.free + permits > self.capacity:
            message = f"released more {self.name} permits than were taken"
            raise ValueError(message)

        self.free += permits

    @property
    def taken(self) -> int:
        return self.capacity - self.free


@dataclass(frozen=True)
class Demand:
    cores: int
    memory_mb: int


class Resources:
    """
    The two semaphores together. A demand is taken from both or from neither.
    """

    def __init__(self, cores: int, memory_mb: int) -> None:
        self.cores = CountingSemaphore("core", cores)
        self.memory = CountingSemaphore("memory", memory_mb)

    def fit(self, demand: Demand) -> Demand:
        """
        The demand cut down to what exists, so a job bigger than the whole
        machine still runs, alone.
        """
        cores = min(demand.cores, self.cores.capacity)
        memory_mb = min(demand.memory_mb, self.memory.capacity)

        return Demand(cores, memory_mb)

    def try_acquire(self, demand: Demand) -> bool:
        if not self.cores.try_acquire(demand.cores):
            return False

        if not self.memory.try_acquire(demand.memory_mb):
            # all or nothing, so the cores go back
            self.cores.release(demand.cores)
            return False

        return True

    def release(self, demand: Demand) -> None:
        self.cores.release(demand.cores)
        self.memory.release(demand.memory_mb)

    def describe(self) -> str:
        cores = f"{self.cores.taken}/{self.cores.capacity} cores"
        memory = f"{self.memory.taken}/{self.memory.capacity} MB"

        return f"{cores}, {memory}"


def system_resources(
    cores: int | None = None, memory_mb: int | None = None
) -> Resources:
    """
    Semaphores sized to this machine, unless a size is given.
    """
    if cores is None:
        cores = max(1, (os.cpu_count() or 2) - CORES_RESERVED)

    if memory_mb is None:
        spare = available_memory_mb() - MEMORY_RESERVE_MB
        memory_mb = max(1, int(spare * MEMORY_SHARE))

    return Resources(cores, memory_mb)


def available_memory_mb() -> int:
    """
    Memory the machine could give out right now, in megabytes.
    """
    if psutil is not None:
        return psutil.virtual_memory().available // 2**20

    if sys.platform == "win32":
        return _windows_available_mb()

    pages = os.sysconf("SC_AVPHYS_PAGES")
    page_size = os.sysconf("SC_PAGE_SIZE")

    return pages * page_size // 2**20


def _windows_available_mb() -> int:
    class MemoryStatus(ctypes.Structure):
        _fields_ = [
            ("length", ctypes.c_ulong),
            ("memory_load", ctypes.c_ulong),
            ("total_physical", ctypes.c_ulonglong),
            ("available_physical", ctypes.c_ulonglong),
            ("total_page_file", ctypes.c_ulonglong),
            ("available_page_file", ctypes.c_ulonglong),
            ("total_virtual", ctypes.c_ulonglong),
            ("available_virtual", ctypes.c_ulonglong),
            ("available_extended_virtual", ctypes.c_ulonglong),
        ]

    status = MemoryStatus()
    status.length = ctypes.sizeof(MemoryStatus)
    ctypes.windll.kernel32.GlobalMemoryStatusEx(ctypes.byref(status))

    return status.available_physical // 2**20
