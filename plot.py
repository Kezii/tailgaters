#!/usr/bin/env python3

import sys
import os
import numpy as np
import pandas as pd
import matplotlib.pyplot as plt
from scipy.interpolate import griddata

from watchdog.observers import Observer
from watchdog.events import FileSystemEventHandler

file_changed = False  # Global or module-level flag

def interpolate_and_plot(csv_file):
    """
    Reads the CSV file, interpolates power over azimuth/elevation,
    and displays a heatmap (with sample points).
    """
    # Attempt to load the file
    try:
        df = pd.read_csv(csv_file, header=None, names=["time", "power", "azimuth", "elevation"])
    except Exception as e:
        print(f"Failed to read or parse {csv_file}: {e}")
        return

    # If there's insufficient data, skip plotting
    if len(df) < 2:
        print("Not enough data to plot.")
        return

    azimuths = df["azimuth"].to_numpy()
    elevations = df["elevation"].to_numpy()
    power = df["power"].to_numpy()

    # Define a grid for interpolation
    az_min, az_max = azimuths.min(), azimuths.max()
    el_min, el_max = elevations.min(), elevations.max()

    num_points = 100
    az_grid = np.linspace(az_min, az_max, num_points)
    el_grid = np.linspace(el_min, el_max, num_points)
    AZ, EL = np.meshgrid(az_grid, el_grid)

    # Interpolate power data (linear, cubic, or nearest)
    power_grid = griddata(
        (azimuths, elevations),
        power,
        (AZ, EL),
        method="linear"
    )

    # Clear the old figure and re-plot
    plt.clf()

    # Heatmap
    plt.imshow(
        power_grid,
        origin="lower",
        aspect="auto",
        extent=[az_min, az_max, el_min, el_max]
    )
    cbar = plt.colorbar()
    cbar.set_label("Power")

    # Scatter original points
    plt.scatter(azimuths, elevations, marker=".")
    plt.xlabel("Azimuth")
    plt.ylabel("Elevation")
    plt.title("Interpolated Power Heatmap")

    plt.draw()  # Update the plot

class CSVFileChangeHandler(FileSystemEventHandler):
    """
    Custom watchdog handler. We ONLY set a global flag here,
    so we do not block or call Matplotlib in a background thread.
    """
    def __init__(self, target_path):
        super().__init__()
        self.target_path = os.path.abspath(target_path)

    def on_modified(self, event):
        global file_changed

        # Only set the flag if the changed file is the one we're watching
        if os.path.abspath(event.src_path) == self.target_path:
            print(f"File changed: {event.src_path}")
            file_changed = True

def main(csv_file):
    global file_changed

    # Make initial plot
    plt.ion()  # Interactive mode
    interpolate_and_plot(csv_file)

    # Create observer
    event_handler = CSVFileChangeHandler(csv_file)
    observer = Observer()

    watch_dir = os.path.dirname(os.path.abspath(csv_file))
    observer.schedule(event_handler, watch_dir, recursive=False)
    observer.start()

    print(f"Watching file: {csv_file}\nClose the plot window or press Ctrl+C to quit.")

    try:
        # Main loop in the main thread
        while True:
            # If the user closed the figure, exit
            if not plt.fignum_exists(1):
                break

            # If the file changed, re-plot
            if file_changed:
                file_changed = False
                interpolate_and_plot(csv_file)

            plt.pause(0.5)  # run the event loop in main thread
    except KeyboardInterrupt:
        pass
    finally:
        observer.stop()
        observer.join()

if __name__ == "__main__":
    if len(sys.argv) < 2:
        print("Usage: python watch_plot.py <csv_file>")
        sys.exit(1)

    csv_file = sys.argv[1]
    main(csv_file)
