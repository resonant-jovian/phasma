# phasma

**Terminal interface for the [caustic](https://github.com/resonant-jovian/caustic) Vlasov–Poisson solver.**

[![Crates.io](https://img.shields.io/crates/v/phasma.svg)](https://crates.io/crates/phasma)
[![License: GPL-3.0](https://img.shields.io/badge/License-GPL--3.0-blue.svg)](https://www.gnu.org/licenses/gpl-3.0)
[![CI](https://github.com/resonant-jovian/phasma/workflows/CI/badge.svg)](https://github.com/resonant-jovian/phasma/actions)

---

phasma is a terminal application built with [ratatui](https://ratatui.rs/) that provides a full interactive workflow for setting up, running, and monitoring caustic simulations — no GUI or web stack required. Everything runs in your terminal over SSH, in tmux, on a headless compute node, wherever you need it.

The interface is a two-column layout navigated by function keys:

| Left column | Right column |
|---|---|
| **Setup** — model, resolution, solver dropdowns | **Density** — projected density heatmap (braille/half-block) |
| **Diagnostics** — E, M, C₂, virial ratio, ETA | **Energy** — conservation time series line chart |

**Tab bar:** `F1` Setup · `F2` Run · `F3` Density · `F4` Phase-space · `F5` Energy · `F6` Profile · `q` Quit

## Features

### Interactive simulation setup

Configure every parameter from the document spec without editing config files:

- **Model selection** — cycle through initial condition types (Plummer, King, Hernquist, NFW, Zel'dovich, custom) with `Tab`/arrow keys
- **Parameter entry** — inline numeric fields for mass, scale radius, domain extents, resolution, time range
- **Solver selection** — pick representation, Poisson method, advection scheme, splitting method from dropdown menus
- **Validation** — parameter constraints checked live (velocity domain ≥ escape velocity, resolution fits in memory, etc.)

### Live monitoring

While the simulation runs, phasma displays real-time dashboards:

- **Density projection** — 2D heatmap of ∫ρ dz rendered as a braille-dot or half-block character plot, updating every N steps
- **Phase-space slice** — f(x, vx) at a chosen y, z slice, showing stream structure and filamentation
- **Energy plot** — time series of total energy E(t) as a line chart, with conservation error ΔE/E displayed numerically
- **Virial ratio** — 2T/|W| over time, showing approach to equilibrium
- **Casimir C₂** — sensitive numerical diffusion monitor
- **Density profile** — spherically averaged ρ(r) at current timestep
- **Performance** — steps/second, ETA, memory usage, peak grid cell count

### Keyboard-driven workflow

| Key | Action |
|---|---|
| `F1` – `F6` | Switch between tabs (Setup, Run, Density, Phase, Energy, Profile) |
| `Enter` | Confirm selection / start run |
| `Tab` / `Shift+Tab` | Next / previous input field |
| `↑` `↓` | Adjust dropdown selections or scroll |
| `Space` | Pause / resume simulation |
| `s` | Save snapshot to HDF5 |
| `c` | Cycle color map (viridis, inferno, plasma, grayscale) |
| `+` / `-` | Zoom density/phase-space view |
| `x` / `y` / `z` | Change projection axis for density view |
| `[` / `]` | Move phase-space slice plane |
| `l` | Toggle log/linear density scale |
| `q` | Quit (with confirmation if running) |

### Config files

While the TUI is the primary interface, you can also load/save configurations as TOML:

```toml
# caustic-run.toml

[model]
type = "plummer"
mass = 1.0
scale_radius = 1.0

[domain]
spatial_extent = 20.0
velocity_extent = 3.0
spatial_resolution = 64
velocity_resolution = 64
boundary = "isolated"

[solver]
representation = "grid6d"
poisson = "fft_isolated"
advection = "semi_lagrangian"
splitting = "strang"

[time]
t_final = 50.0
dt = "adaptive"
cfl_factor = 0.5

[output]
interval = 1.0
directory = "./output"
format = "hdf5"

[exit]
energy_tolerance = 1e-4
mass_threshold = 0.99
```

```bash
# Load config and run immediately
phasma --config caustic-run.toml --run

# Load config into the TUI for editing
phasma --config caustic-run.toml

# Export current TUI setup to file
# (press 'e' in Setup tab)
```

## Installation

### From crates.io

```bash
cargo install phasma
```

### From source

```bash
git clone https://github.com/resonant-jovian/phasma
cd phasma
cargo build --release
./target/release/phasma
```

### Dependencies

- **caustic** (the solver library) — pulled automatically as a cargo dependency
- **ratatui** + **crossterm** — terminal rendering (no external deps)
- **libhdf5** *(optional)* — only needed if you want HDF5 snapshot output. Install via your package manager (`apt install libhdf5-dev`, `brew install hdf5`, etc.)

## Usage

```bash
# Launch the interactive TUI
phasma

# Launch with a config pre-loaded
phasma --config my_run.toml

# Launch and immediately start running (headless-friendly)
phasma --config my_run.toml --run

# Batch mode: no TUI, just run and write output (for HPC jobs)
phasma --config my_run.toml --batch --output ./results/
```

### Batch mode

For HPC / job scheduler environments where you don't want a TUI at all:

```bash
#!/bin/bash
#SBATCH --job-name=phasma
#SBATCH --time=24:00:00
#SBATCH --mem=64G

phasma --config plummer_64.toml --batch --output $SCRATCH/run_001/
```

Batch mode writes snapshots and a `diagnostics.csv` time series to the output directory. No terminal interaction required.

## Project structure

```
phasma/
├── Cargo.toml
├── README.md
├── src/
│   ├── main.rs              # Entry point, arg parsing, app loop
│   ├── app.rs               # Application state machine
│   ├── ui/
│   │   ├── mod.rs
│   │   ├── setup.rs          # Setup tab: model/solver parameter forms
│   │   ├── run.rs            # Run tab: controls + live diagnostics
│   │   ├── density.rs        # Density projection heatmap
│   │   ├── phase.rs          # Phase-space slice view
│   │   ├── energy.rs         # Energy/conservation time series
│   │   └── profile.rs        # Radial density profile plot
│   ├── config.rs             # TOML config serialization
│   ├── runner.rs             # Simulation thread management
│   ├── plotting.rs           # Braille/block char rendering helpers
│   └── colormap.rs           # Terminal color map implementations
├── configs/
│   ├── plummer_64.toml       # Example: Plummer sphere at 64³
│   ├── merger_simple.toml    # Example: two-body merger
│   ├── jeans_test.toml       # Example: Jeans instability validation
│   └── zeldovich_1d.toml     # Example: Zel'dovich pancake
└── tests/
    ├── config_roundtrip.rs
    └── ui_smoke.rs
```

## Relationship to caustic (the library)

phasma is a **consumer** of the `caustic` library. It provides no solver logic itself — it constructs a `caustic::Simulation` from user input, runs it on a background thread, and renders the diagnostics that `caustic` produces.

If you want to embed caustic in your own application, script, or pipeline, use the library directly. phasma is for interactive exploration, quick parameter sweeps, and monitoring long-running jobs from a terminal.

**`caustic` (lib)** ← depends on ← **`phasma` (bin)**

| caustic provides | phasma provides |
|---|---|
| Simulation engine | ratatui UI |
| Phase-space representations | Config loading (TOML) |
| Poisson solvers | Real-time plots |
| Advection schemes | Parameter forms |
| Diagnostics API | Snapshot triggers |
| HDF5 I/O | Batch mode runner |

## Minimum supported Rust version

phasma targets **stable Rust 1.75+**.

## License

This project is licensed under the [GNU General Public License v3.0](https://www.gnu.org/licenses/gpl-3.0.en.html). See [LICENSE](LICENSE) for details.
