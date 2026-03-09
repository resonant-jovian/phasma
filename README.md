# phasma

**Terminal interface for the [caustic](https://github.com/resonant-jovian/caustic) Vlasov–Poisson solver.**

[![Crates.io](https://img.shields.io/crates/v/phasma.svg)](https://crates.io/crates/phasma)
[![License: GPL-3.0](https://img.shields.io/badge/License-GPL--3.0-blue.svg)](https://www.gnu.org/licenses/gpl-3.0)
[![CI](https://github.com/resonant-jovian/phasma/workflows/CI/badge.svg)](https://github.com/resonant-jovian/phasma/actions)

---

phasma is a terminal application built with [ratatui](https://ratatui.rs/) that provides a full interactive workflow for setting up, running, and monitoring caustic simulations — no GUI or web stack required. Everything runs in your terminal over SSH, in tmux, on a headless compute node, wherever you need it.

## Supported models

phasma wires up every implemented caustic component:

| Model | Description | Config key |
|---|---|---|
| Plummer | Isotropic sphere with analytic DF | `plummer` |
| Hernquist | Galaxy model with closed-form DF | `hernquist` |
| King | Tidally truncated (Poisson-Boltzmann ODE) | `king` |
| NFW | Dark matter halo (numerical Eddington inversion) | `nfw` |
| Zel'dovich | Single-mode cosmological pancake | `zeldovich` |
| Merger | Two-body superposition of any equilibrium ICs | `merger` |
| Custom file | User-provided 6D array (.npy) | `custom_file` |

### Initial conditions reference

| Model | Required parameters | Optional sub-table | Physical setup | Expected result |
|---|---|---|---|---|
| `plummer` | `total_mass`, `scale_radius` | — | Isotropic sphere, f(E) = (24sqrt(2)/7pi^3) M (-E)^(7/2) / (a^3 (GM/a)^5) | Stable equilibrium; density profile rho ~ (1 + r^2/a^2)^(-5/2). Energy drift < 1% over many dynamical times at 16^3. |
| `hernquist` | `total_mass`, `scale_radius` | — | Galaxy model with closed-form DF (eq. 17 of Hernquist 1990). Cuspy center rho ~ r^(-1). | Stable equilibrium; density rho ~ r^(-1)(r+a)^(-3). Steeper central density than Plummer. |
| `king` | `total_mass`, `scale_radius` | `[model.king]` with `w0` (dimensionless central potential, typical 3-9) | Tidally truncated model. Poisson-Boltzmann ODE integrated with RK4. Finite tidal radius r_t. | Truncated isothermal profile. Higher W0 = more concentrated. W0=6 gives r_t/r_0 ~ 30. Density drops to zero at tidal radius. |
| `nfw` | `total_mass`, `scale_radius` | `[model.nfw]` with `concentration` (c = r_vir/r_s, typical 5-20) | Navarro-Frenk-White dark matter halo profile. DF computed via numerical Eddington inversion. | Cuspy profile rho ~ r^(-1)(1 + r/r_s)^(-2). Higher concentration = more centrally concentrated. |
| `zeldovich` | `total_mass`, `scale_radius` | `[model.zeldovich]` with `amplitude` (perturbation strength, 0.1-1.0) and `wave_number` (mode number) | Single-mode cosmological pancake. Cold dark matter sheet with sinusoidal perturbation. | Caustic (density singularity) forms at t_caustic = 1/(amplitude * wave_number). Sheet folding produces multi-stream regions. |
| `merger` | `total_mass`, `scale_radius` | `[model.merger]` with `separation` (initial distance) and `mass_ratio` (m2/m1) | Two Plummer spheres placed at +/- separation/2 along x-axis. Collisionless superposition f = f1 + f2. | Bodies fall toward each other, pass through (collisionless), oscillate, and eventually merge into a virialized remnant. |
| `custom_file` | `total_mass`, `scale_radius` | `[model.custom_file]` with `file_path` (path to .npy 6D array) | User-provided distribution function on the 6D grid. Array shape must match domain resolution. | Depends entirely on the input data. |

**Poisson solvers:** `fft_periodic` (FFT with periodic BC), `fft_isolated` (Hockney-Eastwood zero-padded)

**Time integrators:** `strang` (2nd-order), `yoshida` (4th-order), `lie` (1st-order)

**Exit conditions:** time limit, energy drift, mass loss, Casimir drift, wall-clock limit, steady state, virial equilibrium, CFL violation, caustic formation

## Features

### Interactive simulation setup

Configure every parameter from the TUI without editing config files:

- **Model selection** — cycle through initial condition types with `Tab`/arrow keys
- **Parameter entry** — inline numeric fields for mass, scale radius, domain extents, resolution, time range
- **Solver selection** — pick Poisson method, advection scheme, and integrator from dropdown menus
- **Validation** — constraints checked live (velocity domain vs escape velocity, resolution vs memory budget, required sub-config fields)

### Live monitoring

While the simulation runs, phasma displays real-time dashboards across 10 tabs:

- **F2 Run Control** — progress gauge, density and phase-space thumbnails, energy conservation chart, diagnostics sidebar with virial ratio 2T/|W|
- **F3 Density** — 2D heatmap of projected density with axis selection and zoom
- **F4 Phase Space** — f(x_i, v_j) marginal projections for all 9 dimension pairs, with zoom
- **F5 Energy** — conservation time series: E(t), T(t), W(t), drift ΔE/E₀, mass drift, Casimir drift, entropy
- **F6 Rank** — rank monitoring (placeholder)
- **F7 Profiles** — spherically averaged ρ(r), velocity dispersion, mass profile, circular velocity, anisotropy β(r)
- **F8 Performance** — step timing, adaptive timestep evolution, cumulative wall time, throughput stats
- **F9 Poisson** — Poisson solver detail (placeholder)
- **F10 Settings** — theme and colormap selection

### History scrubbing

phasma stores the last 100 simulation snapshots in a ring buffer. Use `←`/`→` to scrub backward/forward through history on any visualization tab. Press `Backspace` to jump back to live.

### Keyboard controls

| Key | Action |
|---|---|
| `F1` – `F10` | Switch tabs (Setup, Run, Density, Phase, Energy, Rank, Profiles, Perf, Poisson, Settings) |
| `Tab` / `Shift+Tab` | Next / previous tab |
| `Space` | Pause / resume simulation (global) |
| `←` / `→` | Scrub backward / forward through history |
| `Backspace` | Jump to live (exit scrub mode) |
| `?` | Toggle help overlay |
| `q` | Quit (with confirmation if sim is running) |

**Density (F3):**

| Key | Action |
|---|---|
| `x` / `y` / `z` | Change projection axis |
| `+` / `-` / scroll | Zoom in / out |
| `r` | Reset zoom |
| `l` | Toggle log scale |
| `c` | Cycle colormap |
| `i` | Toggle info bar |

**Phase Space (F4):**

| Key | Action |
|---|---|
| `1`-`3` | Select spatial dimension (x, y, z) |
| `4`-`6` | Select velocity dimension (vx, vy, vz) |
| `+` / `-` / scroll | Zoom in / out |
| `r` / `0` | Reset zoom |
| `l` | Toggle log scale |
| `c` | Cycle colormap |
| `i` | Toggle info bar |

**Energy (F5):**

| Key | Action |
|---|---|
| `t` / `k` / `w` | Toggle traces: total energy / kinetic / potential |
| `d` | Toggle drift view (ΔE/E₀) |
| `1`-`4` | Select panel (energy, mass, Casimir, entropy) |

**Run Control (F2):**

| Key | Action |
|---|---|
| `p` / `Space` | Pause / resume |
| `s` | Stop simulation |
| `r` | Restart simulation |
| `1`-`3` | Log filter: all / warn+ / error only |

**Profiles (F7):**

| Key | Action |
|---|---|
| `1`-`5` | Select profile: density, dispersion, mass, v_circ, anisotropy |
| `l` | Toggle log scale |
| `a` | Toggle analytic overlay |

**Global:**

| Key | Action |
|---|---|
| `e` | Open export menu (`1`-`9` to quick-select format) |
| `T` | Cycle theme |
| `C` | Cycle colormap (global) |

## Preset configurations

phasma ships with 8 preset configurations covering common use cases:

| Preset | Model | Grid | Integrator | Use case |
|---|---|---|---|---|
| `speed_priority` | Plummer | 8³×8³ | Strang | Fast iteration, smoke tests |
| `resolution_priority` | Plummer | 32³×32³ | Yoshida | High-accuracy production runs |
| `conservation_priority` | Plummer | 16³×16³ | Yoshida | Conservation law validation |
| `cosmological` | Zel'dovich | 32³×32³ | Strang | Caustic formation |
| `king_equilibrium` | King (W₀=6) | 16³×16³ | Strang | Tidally truncated equilibrium |
| `nfw_dark_matter` | NFW (c=10) | 16³×16³ | Yoshida | Dark matter halo |
| `hernquist_galaxy` | Hernquist | 16³×16³ | Yoshida | Galaxy model |
| `merger` | 2× Plummer | 16³×16³ | Strang | Two-body interaction |

```bash
# Run a preset directly
phasma --config configs/speed_priority.toml --run

# Batch mode (headless)
phasma --config configs/cosmological.toml --batch
```

## Config files

While the TUI is the primary interface, you can load and save configurations as TOML:

```toml
[model]
type = "plummer"
total_mass = 1.0
scale_radius = 1.0

[domain]
spatial_extent = 10.0
velocity_extent = 3.0
spatial_resolution = 16
velocity_resolution = 16
boundary = "periodic|truncated"
coordinates = "cartesian"
gravitational_constant = 1.0

[solver]
representation = "uniform"
poisson = "fft_periodic"
advection = "semi_lagrangian"
integrator = "yoshida"

[time]
t_final = 20.0
dt_mode = "adaptive"
cfl_factor = 0.5

[output]
directory = "output"
snapshot_interval = 1.0

[exit]
energy_drift_tolerance = 0.05
mass_drift_tolerance = 0.01
```

Models with sub-parameters use TOML sub-tables:

```toml
[model]
type = "king"
total_mass = 1.0
scale_radius = 1.0

[model.king]
w0 = 6.0
```

```toml
[model]
type = "nfw"
total_mass = 1.0
scale_radius = 1.0

[model.nfw]
concentration = 10.0
```

```toml
[model]
type = "merger"
total_mass = 2.0
scale_radius = 1.0

[model.merger]
separation = 6.0
mass_ratio = 1.0
```

```toml
[model]
type = "zeldovich"
total_mass = 1.0
scale_radius = 1.0

[model.zeldovich]
amplitude = 0.3
wave_number = 1.0
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

### Man page

```bash
phasma --generate-man | sudo tee /usr/local/share/man/man1/phasma.1 > /dev/null
sudo mandb
```

### Dependencies

- **caustic** (the solver library) — pulled automatically as a cargo dependency
- **ratatui** + **crossterm** — terminal rendering (no external deps)

## Usage

```bash
# Launch the interactive TUI
phasma

# Launch with a config pre-loaded
phasma --config my_run.toml

# Launch and immediately start running
phasma --config my_run.toml --run

# Batch mode: no TUI, just run and print progress to stderr
phasma --config my_run.toml --batch
```

### CLI flags

| Flag | Argument | Description |
|---|---|---|
| `-c`, `--config` | `PATH` | Path to simulation config file (TOML) |
| `--run` | — | Start simulation immediately on launch |
| `--batch` | — | Headless batch mode — run without TUI (for HPC / job schedulers) |
| `--generate-man` | — | Generate a man page and print to stdout |
| `-h`, `--help` | — | Print help (includes model/solver/integrator listing) |
| `-V`, `--version` | — | Print version |

### Planned CLI flags (not yet implemented)

| Flag | Argument | Description |
|---|---|---|
| `--playback` | `DIR` | Replay recorded snapshots from a previous run directory |
| `--compare` | `DIR DIR` | Side-by-side comparison of two simulation runs |
| `--sweep` | `TOML` | Parameter sweep mode — vary parameters across a grid |
| `--convergence` | `TOML` | Convergence study — run at increasing resolutions |
| `--regression-test` | `DIR` | CI-compatible regression test against a reference directory |
| `--monitor` | `DIR` | Monitor a running batch job by watching its output directory |
| `--tail` | `PATH` | Tail a recording directory, auto-advancing as snapshots appear |
| `--wizard` | — | Guided first-run wizard |
| `--save-preset` | `NAME` | Save the current configuration as a named preset |
| `--batch-compare` | `DIR DIR [...]` | Batch comparison report across multiple runs |
| `--report` | `PATH` | Output path for batch comparison report |

### Batch mode

For HPC / job scheduler environments where you don't want a TUI:

```bash
#!/bin/bash
#SBATCH --job-name=phasma
#SBATCH --time=24:00:00
#SBATCH --mem=64G

phasma --config configs/resolution_priority.toml --batch
```

Batch mode prints timestep progress to stderr and writes output to the configured directory.

## Project structure

```
phasma/
├── Cargo.toml
├── README.md
├── configs/                    # Preset TOML configurations
│   ├── speed_priority.toml
│   ├── resolution_priority.toml
│   ├── conservation_priority.toml
│   ├── cosmological.toml
│   ├── king_equilibrium.toml
│   ├── nfw_dark_matter.toml
│   ├── hernquist_galaxy.toml
│   └── merger.toml
└── src/
    ├── main.rs                 # Entry point, arg parsing, batch mode
    ├── sim.rs                  # caustic integration (IC/solver/integrator dispatch)
    ├── config/
    │   └── mod.rs              # PhasmaConfig schema (serde, all sections)
    ├── toml.rs                 # Legacy two-stage config (string → Decimal)
    ├── tui/
    │   ├── app.rs              # Application state machine
    │   ├── cli.rs              # CLI argument definitions (clap)
    │   ├── tabs/               # Tab implementations (setup, run_control, density, ...)
    │   ├── widgets/            # Reusable widgets (heatmap, colorbar, sparkline table)
    │   ├── status_bar.rs       # Bottom status bar (ETA, throughput, memory)
    │   ├── help.rs             # Help overlay
    │   ├── export_menu.rs      # Export format selector
    │   └── ...
    ├── data/
    │   └── live.rs             # Live data provider (diagnostics store, scrub history)
    ├── export/                 # Export formats (CSV, JSON, NPY, Parquet, VTK, ZIP)
    ├── colormaps/              # Terminal colormap implementations
    └── themes.rs               # Color themes (dark, light, solarized, ...)
```

## Relationship to caustic

phasma is a **consumer** of the `caustic` library. It provides no solver logic — it constructs a `caustic::Simulation` from user input, runs it on a background thread, and renders the diagnostics that `caustic` produces.

If you want to embed caustic in your own application, script, or pipeline, use the library directly. phasma is for interactive exploration and monitoring long-running jobs from a terminal.

**`caustic` (lib)** ← depends on ← **`phasma` (bin)**

| caustic provides | phasma provides |
|---|---|
| Simulation engine | ratatui TUI |
| Phase-space representations | Config loading (TOML presets) |
| Poisson solvers (FFT periodic/isolated) | Real-time density/phase-space heatmaps |
| Advection (semi-Lagrangian) | Energy conservation charts |
| Time integrators (Strang/Yoshida/Lie) | History scrubbing |
| Diagnostics API | Batch mode for HPC |
| Exit conditions | Export (CSV, JSON, NPY, Parquet, VTK, ZIP) |

## Minimum supported Rust version

phasma targets **stable Rust 1.85+** (edition 2024).

## License

This project is licensed under the [GNU General Public License v3.0](https://www.gnu.org/licenses/gpl-3.0.en.html). See [LICENSE](LICENSE) for details.
