# phasma

**Terminal interface for the [caustic](https://github.com/resonant-jovian/caustic) Vlasov-Poisson solver.**

[![Crates.io](https://img.shields.io/crates/v/phasma.svg)](https://crates.io/crates/phasma)
[![License: GPL-3.0](https://img.shields.io/badge/License-GPL--3.0-blue.svg)](https://www.gnu.org/licenses/gpl-3.0)

---

phasma is a terminal application built with [ratatui](https://ratatui.rs/) for setting up, running, and monitoring 6D Vlasov-Poisson simulations using the caustic solver library. It runs entirely in the terminal — over SSH, in tmux, on headless compute nodes.

It provides real-time density and phase-space heatmaps, energy conservation charts, radial profiles, performance dashboards, history scrubbing, batch execution, parameter sweeps, convergence studies, playback, comparison, and export — all without leaving the terminal.

> **Note:** This application has not yet reached version 0.1.0. The interface and configuration format are unstable, features may be incomplete or change without notice, and it is not yet intended for general use. Even after 0.1.0, until version 1.0.0 it should not be relied upon for production workloads or serious research.

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
phasma --generate-man > phasma.1
sudo mv phasma.1 /usr/share/man/man1/
```

### Dependencies

- **caustic** — the solver library, pulled automatically as a cargo dependency
- **ratatui** + **crossterm** — terminal rendering (no external deps)
- Rust 1.85+ (edition 2024)

## Quick start

```bash
# Launch the TUI — browse and load configs from the Setup tab
phasma

# Load a preset and start immediately
phasma --config configs/balanced.toml --run

# Headless batch mode for HPC / SLURM
phasma --config configs/nfw_high_res.toml --batch

# Verbose output — see every build step and per-step diagnostics
phasma --config configs/balanced.toml --run --verbose

# Replay a completed batch run
phasma --playback output/run_20260310_143022/
```

A minimal config:

```toml
[model]
type = "plummer"
```

All fields have sensible defaults. Smart defaults auto-fill boundary conditions, Poisson solver, domain extent, and t_final based on model type.

## Modes of operation

| Mode | Flag | Description |
|---|---|---|
| Interactive TUI | (default) | Browse configs, run, monitor live |
| Auto-run | `--run` | Load config and start sim immediately |
| Batch | `--batch` | Headless, saves to disk, suitable for HPC |
| Playback | `--playback DIR` | Replay saved snapshots with scrubbing |
| Comparison | `--compare DIR DIR` | Side-by-side two-run comparison |
| Monitor | `--monitor DIR` | Watch a running batch job live |
| Tail | `--tail DIR` | Like monitor, auto-advances |
| Wizard | `--wizard` | Guided interactive config generation |
| Sweep | `--sweep TOML` | Parameter sweep (Cartesian product) |
| Convergence | `--convergence TOML` | Resolution convergence study |
| Regression | `--regression-test DIR` | CI-compatible regression test (exit 0/1) |
| Batch compare | `--batch-compare DIR...` | Markdown comparison report |

## CLI reference

| Flag | Argument | Description |
|---|---|---|
| `-c`, `--config` | `PATH` | Path to simulation config file (TOML) |
| `--run` | — | Start simulation immediately |
| `--batch` | — | Headless batch mode |
| `-v`, `--verbose` | — | Detailed logging of every build step and per-step diagnostics. In batch mode prints to stderr; in TUI mode appears in the F2 log panel |
| `--playback` | `DIR` | Replay saved snapshots in TUI |
| `--compare` | `DIR DIR` | Side-by-side comparison of two runs |
| `--sweep` | `TOML` | Parameter sweep |
| `--convergence` | `TOML` | Convergence study |
| `--regression-test` | `DIR` | Regression test (exit 0 on pass, 1 on fail) |
| `--monitor` | `DIR` | Watch a batch job's output directory |
| `--tail` | `DIR` | Like `--monitor`, auto-advances to latest |
| `--wizard` | — | Interactive config wizard |
| `--save-preset` | `NAME` | Save loaded config as a named preset |
| `--batch-compare` | `DIR DIR [...]` | Markdown comparison report across runs |
| `--report` | `PATH` | Output path for `--batch-compare` (default: `comparison_report.md`) |
| `--generate-man` | — | Print roff man page to stdout |

### Verbose mode

`--verbose` / `-v` enables detailed logging during simulation startup and stepping:

```
  [verbose] Loading config from: configs/balanced.toml
  [verbose] Config loaded: model=plummer, repr=uniform, poisson=fft_isolated, integrator=yoshida
  [verbose] Domain: spatial_extent=10, velocity_extent=3, N_x=16, N_v=16
  [verbose] Phase-space grid: 16^3 × 16^3 = 16777216 cells (128.0 MB)
  [verbose] Boundary conditions: isolated|truncated
  [verbose] Building domain...
  [verbose] Domain built in 0.3 ms
  [verbose] Building IC: model=plummer, M=1, a=1
  [verbose] IC sampled in 1842.1 ms — 8388608 non-zero cells out of 16777216
  [verbose] Building phase-space representation: uniform
  [verbose] Representation built in 0.0 ms
  [verbose] Building Poisson solver: fft_isolated
  [verbose] Poisson solver built in 12.4 ms
  [verbose] Building integrator: yoshida
  [verbose] Assembling simulation: G=1, t_final=15, cfl=0.35, conservation=none
  [verbose] Simulation assembled in 45.3 ms
  [verbose] Exit conditions wired: 3 active (energy_drift_tol=0.5, mass_drift_tol=0.1)
  [verbose] Build complete in 1902.8 ms
```

In TUI mode these messages appear in the F2 Run Control tab log panel. Per-step verbose messages show step number, time, timestep, wall time, and energy drift.

### Batch output directory layout

```
output/<prefix>_YYYYMMDD_HHMMSS/
  config.toml          -- copy of input config
  diagnostics.csv      -- time series (appended each step)
  snapshots/
    state_000000.json   -- periodic SimState snapshots
    state_000001.json
    ...
    state_final.json    -- last state
  metadata.json         -- version, timing, exit reason, snapshot count
```

This directory is the input for `--playback`, `--monitor`, `--compare`, `--regression-test`, and `--batch-compare`.

## TUI tabs

| Tab | Key | Content |
|---|---|---|
| Setup | F1 | Config browser, structured config summary, memory estimate, preset selector (Ctrl+P) |
| Run Control | F2 | Progress gauge, aspect-corrected density/phase-space thumbnails, energy chart, diagnostics sidebar, log stream |
| Density | F3 | 2D projected density heatmap with aspect correction, axis selection (x/y/z), zoom, log scale, contour overlay |
| Phase Space | F4 | f(x_i, v_j) marginal projections for all 9 dimension pairs, physical aspect ratio (default on, `p` to toggle), data cursor |
| Energy | F5 | Conservation time series: E(t), T(t), W(t), drift, mass, Casimir, entropy — 4 panels |
| Rank | F6 | HT/TT rank evolution, per-node table, singular value spectrum, truncation error (dimmed when repr=uniform) |
| Profiles | F7 | Radial density, velocity dispersion, enclosed mass, circular velocity, anisotropy, Lagrangian radii, analytic overlays |
| Performance | F8 | Step timing breakdown, adaptive timestep chart, memory breakdown, cumulative wall time |
| Poisson | F9 | Poisson residual, potential power spectrum, Green's function detail (dimmed when poisson != fft_isolated) |
| Settings | F10 | Theme and colormap selection |

## Keyboard controls

### Global

| Key | Action |
|---|---|
| `F1`–`F10` | Switch tabs |
| `Tab` / `Shift+Tab` | Next / previous tab |
| `Space` | Pause / resume simulation |
| `Left` / `Right` | Scrub backward / forward through history |
| `Backspace` | Jump to live (exit scrub mode) |
| `?` | Toggle help overlay |
| `e` | Open export menu |
| `T` | Cycle theme |
| `C` | Cycle colormap |
| `Ctrl+S` | Save current config to TOML file |
| `Ctrl+O` | Load config (jump to Setup tab) |
| `/` | Jump-to-time dialog |
| `:` | Command palette |
| `a` | Add annotation at current time |
| `Ctrl+B` | Toggle bookmark panel |
| `q` | Quit (with confirmation if sim is running) |

### Run Control (F2)

| Key | Action |
|---|---|
| `p` / `Space` | Pause / resume |
| `s` | Stop simulation |
| `r` | Restart simulation |
| `1`–`3` | Log filter: all / warn+ / error only |

### Density (F3)

| Key | Action |
|---|---|
| `x` / `y` / `z` | Change projection axis |
| `+` / `-` / scroll | Zoom in / out |
| `r` | Reset zoom |
| `l` | Toggle log scale |
| `c` | Cycle colormap |
| `i` | Toggle info bar |

### Phase Space (F4)

| Key | Action |
|---|---|
| `1`–`3` | Select spatial dimension (x, y, z) |
| `4`–`6` | Select velocity dimension (vx, vy, vz) |
| `+` / `-` / scroll | Zoom in / out |
| `r` / `0` | Reset zoom |
| `l` | Toggle log scale |
| `c` | Cycle colormap / comparison view |
| `p` | Toggle physical aspect ratio (default on) |
| `s` | Toggle stream-count overlay |
| `i` | Toggle info bar |

### Energy (F5)

| Key | Action |
|---|---|
| `t` / `k` / `w` | Toggle traces: total / kinetic / potential |
| `d` | Toggle drift view |
| `1`–`4` | Select panel: energy, mass, Casimir, entropy |

### Profiles (F7)

| Key | Action |
|---|---|
| `1`–`5` | Select profile: density, dispersion, mass, v_circ, anisotropy |
| `l` | Toggle log scale |
| `a` | Toggle analytic overlay |
| `b` | Adjust bin count |

### Playback keys (when in playback mode)

| Key | Action |
|---|---|
| `[` / `]` | Step backward / forward one frame |
| `{` / `}` | Jump 10 frames |
| `Home` / `End` | Jump to start / end |
| `<` / `>` | Decrease / increase playback speed |

## Config file reference

All sections and fields are optional — sensible defaults are provided. The full config has 10 top-level sections.

### `[domain]` — Simulation domain

| Key | Type | Default | Description |
|---|---|---|---|
| `spatial_extent` | float | `10.0` | Half-width of the spatial box. Domain spans [-L, L]^3. |
| `velocity_extent` | float | `5.0` | Half-width of the velocity box. Domain spans [-V, V]^3. |
| `spatial_resolution` | integer | `8` | Grid cells per spatial dimension. Must be power-of-2 for FFT solvers. |
| `velocity_resolution` | integer | `8` | Grid cells per velocity dimension. Must be power-of-2 for FFT solvers. |
| `boundary` | string | `"periodic\|truncated"` | Spatial BC \| velocity BC. Options: `periodic`, `isolated`, `reflecting` \| `truncated`, `open` |
| `coordinates` | string | `"cartesian"` | Coordinate system |
| `gravitational_constant` | float | `1.0` | Value of G |

Memory: `N_x^3 * N_v^3 * 8 bytes`. A 16^3 x 16^3 grid = 128 MB. A 32^3 x 32^3 grid = 8 GB.

```toml
[domain]
spatial_extent = 10.0
velocity_extent = 3.0
spatial_resolution = 16
velocity_resolution = 16
boundary = "isolated|truncated"
gravitational_constant = 1.0
```

---

### `[model]` — Initial conditions

| Key | Type | Default | Description |
|---|---|---|---|
| `type` | string | `"plummer"` | IC model type |
| `total_mass` | float | `1.0` | Total system mass |
| `scale_radius` | float | `1.0` | Characteristic scale radius |

#### Sub-tables for specific models

**`[model.king]`** — Tidally truncated King model

| Key | Required | Description |
|---|---|---|
| `w0` | yes | Dimensionless central potential (typical 3.0–9.0) |

**`[model.nfw]`** — NFW dark matter halo

| Key | Required | Description |
|---|---|---|
| `concentration` | yes | c = r_vir / r_s (typical 5–20) |
| `virial_mass` | no | Virial mass (default: total_mass) |
| `velocity_anisotropy` | no | `"isotropic"` or beta value |

**`[model.zeldovich]`** — Zel'dovich pancake

| Key | Required | Description |
|---|---|---|
| `amplitude` | yes | Perturbation amplitude (0.1–1.0) |
| `wave_number` | yes | Mode wave number |

**`[model.merger]`** — Two-body merger

| Key | Required | Description |
|---|---|---|
| `separation` | yes | Initial separation distance |
| `mass_ratio` | yes | m2/m1 (1.0 = equal mass) |
| `relative_velocity` | no | Relative velocity (default: 0) |
| `impact_parameter` | no | Impact parameter (default: 0) |
| `scale_radius_1` | no | Scale radius of body 1 |
| `scale_radius_2` | no | Scale radius of body 2 |

**`[model.tidal]`** — Tidal stream

| Key | Required | Description |
|---|---|---|
| `progenitor_type` | yes | `"plummer"`, `"hernquist"`, `"king"`, `"nfw"` |
| `progenitor_mass` | yes | Progenitor mass |
| `progenitor_scale_radius` | yes | Progenitor scale radius |
| `progenitor_position` | yes | [x, y, z] position |
| `progenitor_velocity` | yes | [vx, vy, vz] velocity |
| `host_type` | yes | `"point_mass"`, `"nfw_fixed"`, `"logarithmic"` |
| `host_mass` | yes | Host mass |
| `host_scale_radius` | yes | Host scale radius |

**`[model.uniform_perturbation]`** — Perturbed uniform Maxwellian

| Key | Required | Description |
|---|---|---|
| `background_density` | yes | Mean density |
| `velocity_dispersion` | yes | Thermal velocity dispersion |
| `perturbation_amplitude` | yes | Perturbation amplitude |
| `perturbation_wavenumber` | yes | [kx, ky, kz] wave vector |

**`[model.disk]`** — Exponential disk

| Key | Required | Description |
|---|---|---|
| `disk_mass` | no | Disk mass |
| `disk_scale_length` | no | Scale length |
| `radial_velocity_dispersion` | no | Radial velocity dispersion |

**`[model.custom_file]`** — Custom 6D array

| Key | Required | Description |
|---|---|---|
| `file_path` | yes | Path to .npy file |
| `format` | yes | `"npy"` |

---

### `[solver]` — Numerical methods

| Key | Type | Default | Description |
|---|---|---|---|
| `representation` | string | `"uniform"` | Phase-space representation |
| `poisson` | string | `"fft_periodic"` | Poisson solver |
| `advection` | string | `"semi_lagrangian"` | Advection scheme |
| `integrator` | string | `"strang"` | Time integrator |
| `conservation` | string | `"none"` | Conservation scheme |

#### Phase-space representations

| Value | Description | Memory |
|---|---|---|
| `uniform` / `uniform_grid` | Full 6D grid | O(N^6) |
| `hierarchical_tucker` / `ht` | HT tensor decomposition | O(N^3 r^3) |
| `tensor_train` | Tensor-train decomposition | O(N r^2 d) |
| `sheet_tracker` | Lagrangian sheet tracker | O(N^3) |
| `spectral` / `velocity_ht` | Hermite velocity basis | O(N^3 M) |
| `amr` | Adaptive mesh refinement | varies |
| `hybrid` | Sheet/grid hybrid | varies |

#### Poisson solvers

| Value | Description | BC |
|---|---|---|
| `fft_periodic` / `fft` | FFT periodic | periodic |
| `fft_isolated` | Hockney-Eastwood zero-padded FFT | isolated |
| `tensor` / `tensor_poisson` | Braess-Hackbusch exponential sum | isolated |
| `multigrid` | V-cycle multigrid (red-black GS) | isolated |
| `spherical` / `spherical_harmonics` | Legendre decomposition + radial ODE | spherical |
| `tree` / `barnes_hut` | Barnes-Hut octree | isolated |

#### Time integrators

| Value | Order | Sub-steps | Description |
|---|---|---|---|
| `strang` | 2nd | 3 | Strang operator splitting (symplectic) |
| `yoshida` | 4th | 7 | Yoshida splitting — best conservation |
| `lie` | 1st | 2 | Lie splitting (simplest, least accurate) |
| `unsplit` / `unsplit_rk4` | 4th | — | Unsplit method-of-lines RK4 |
| `unsplit_rk2` | 2nd | — | Unsplit RK2 |
| `unsplit_rk3` | 3rd | — | Unsplit RK3 |
| `rkei` | 3rd | 3 | SSP-RK3 exponential integrator (unsplit) |

#### HT/TT solver options `[solver.ht]`

| Key | Type | Default | Description |
|---|---|---|---|
| `max_rank` | integer | `100` | Maximum rank per node |
| `initial_rank` | integer | `16` | Initial rank (spectral modes) |
| `tolerance` | float | `1e-6` | HSVD truncation tolerance |

```toml
[solver]
representation = "uniform"
poisson = "fft_isolated"
advection = "semi_lagrangian"
integrator = "yoshida"
conservation = "none"         # or "lomac" for mass/momentum/energy conservation
```

---

### `[time]` — Time stepping

| Key | Type | Default | Description |
|---|---|---|---|
| `t_final` | float | `10.0` | Simulation end time |
| `dt_mode` | string | `"adaptive"` | `"adaptive"` or `"fixed"` |
| `dt_fixed` | float | `0.1` | Fixed timestep |
| `cfl_factor` | float | `0.5` | CFL safety factor (0, 1] |
| `dt_min` | float | `1e-6` | Minimum timestep (adaptive) |
| `dt_max` | float | `1.0` | Maximum timestep (adaptive) |

---

### `[output]` — Output settings

| Key | Type | Default | Description |
|---|---|---|---|
| `directory` | string | `"output"` | Base output directory |
| `prefix` | string | `"run"` | Subdirectory prefix |
| `snapshot_interval` | float | `1.0` | Time between snapshot saves |
| `checkpoint_interval` | float | `10.0` | Time between checkpoints |
| `diagnostics_interval` | float | `0.1` | Time between diagnostics rows |
| `format` | string | `"binary"` | Snapshot format |

---

### `[exit]` — Termination conditions

The simulation exits when **any** enabled condition triggers.

| Key | Type | Default | Description |
|---|---|---|---|
| `energy_drift_tolerance` | float | `0.5` | Max \|Delta E / E_0\| |
| `mass_drift_tolerance` | float | `0.1` | Max \|Delta M / M_0\| |
| `virial_equilibrium` | bool | `false` | Exit when virial ratio stabilizes |
| `virial_tolerance` | float | `0.05` | Virial equilibrium tolerance |
| `wall_clock_limit` | float | none | Max seconds |
| `cfl_violation` | bool | `true` | Exit on CFL violation |
| `steady_state` | bool | `false` | Exit on steady state |
| `steady_state_tolerance` | float | `1e-6` | Steady state threshold |
| `casimir_drift_tolerance` | float | `0.0` | Max Casimir drift (0 = disabled) |
| `caustic_formation` | bool | `false` | Exit when first caustic forms |

---

### `[performance]` — Performance tuning

| Key | Type | Default | Description |
|---|---|---|---|
| `num_threads` | integer | `0` | Rayon threads (0 = all available) |
| `memory_budget_gb` | float | `4.0` | Memory budget for validation warnings |
| `simd` | bool | `true` | Enable SIMD |
| `allocator` | string | `"system"` | `"system"`, `"jemalloc"`, `"mimalloc"` |

---

### `[playback]` — Playback settings

| Key | Type | Default | Description |
|---|---|---|---|
| `source_directory` | string | none | Snapshot directory path |
| `fps` | float | `10.0` | Playback frames per second |
| `loop_playback` | bool | `false` | Loop at end |

---

### `[logging]` — Logging

| Key | Type | Default | Description |
|---|---|---|---|
| `level` | string | `"info"` | Log level (`"trace"`, `"debug"`, `"info"`, `"warn"`, `"error"`) |
| `file` | string | none | Log file path |
| `structured` | bool | `false` | Structured JSON logging |

---

### `[appearance]` — TUI appearance

| Key | Type | Default | Description |
|---|---|---|---|
| `theme` | string | `"dark"` | `"dark"`, `"light"`, `"solarized"`, `"gruvbox"` |
| `colormap_default` | string | `"viridis"` | `"viridis"`, `"inferno"`, `"plasma"`, `"magma"`, `"grayscale"`, `"cubehelix"`, `"coolwarm"` |
| `braille_density` | bool | `true` | Braille-based density rendering |
| `border_style` | string | `"rounded"` | `"rounded"`, `"plain"`, `"double"` |
| `square_pixels` | bool | `true` | Compensate for non-square terminal cells in all heatmaps (F2, F3, F4) |
| `min_columns` | integer | `80` | Minimum terminal width |
| `min_rows` | integer | `24` | Minimum terminal height |

## Preset configurations

phasma ships with 30 preset TOML configurations in `configs/`:

### Equilibrium models

| Preset | Model | Grid | Integrator | Notes |
|---|---|---|---|---|
| `speed_priority` | Plummer | 8^3 x 8^3 | Strang | Fast smoke tests (~2 MB) |
| `default` | Plummer | 16^3 x 16^3 | Strang | Default starting point (~128 MB) |
| `balanced` | Plummer | 16^3 x 16^3 | Yoshida | Good conservation balance |
| `conservation_priority` | Plummer | 16^3 x 16^3 | Yoshida | Conservation law validation |
| `resolution_priority` | Plummer | 32^3 x 32^3 | Yoshida | High-accuracy production (~8 GB) |
| `isolated_plummer` | Plummer | 16^3 x 16^3 | Strang | Isolated BC with fft_isolated |
| `yoshida_plummer` | Plummer | 16^3 x 16^3 | Yoshida | 4th-order integrator comparison |
| `king_equilibrium` | King (W0=6) | 16^3 x 16^3 | Strang | Tidally truncated equilibrium |
| `hernquist_galaxy` | Hernquist | 16^3 x 16^3 | Yoshida | Galaxy model |
| `nfw_dark_matter` | NFW (c=10) | 16^3 x 16^3 | Yoshida | Dark matter halo |
| `nfw_high_res` | NFW (c=10) | 32^3 x 16^3 | Yoshida | High-res NFW (~1 GB) |

### Advanced solvers

| Preset | Model | Solver | Notes |
|---|---|---|---|
| `ht_plummer` | Plummer | HT tensor | Hierarchical Tucker compressed |
| `tensor_train_plummer` | Plummer | TT decomposition | Tensor-train representation |
| `spectral_plummer` | Plummer | Spectral velocity | Hermite velocity basis |
| `lomac_plummer` | Plummer | LoMaC conservation | Mass/momentum/energy preserving |
| `unsplit_rk4_plummer` | Plummer | Unsplit RK4 | Method-of-lines integrator |

### Alternative Poisson solvers

| Preset | Model | Poisson solver | Notes |
|---|---|---|---|
| `tensor_poisson_plummer` | Plummer | Exp-sum tensor | Braess-Hackbusch isolated |
| `multigrid_plummer` | Plummer | V-cycle multigrid | Red-black Gauss-Seidel |
| `spherical_harmonics_plummer` | Plummer | Spherical harmonics | Legendre + radial ODE |
| `tree_nfw` | NFW | Barnes-Hut tree | Octree gravity |

### Multi-body and cosmological

| Preset | Model | Notes |
|---|---|---|
| `merger` | 2x Plummer (equal mass) | Head-on collision |
| `merger_demo` | 2x Plummer | Demonstration merger |
| `merger_unequal` | 2x Plummer (3:1) | Unequal mass ratio |
| `cosmological` | Zel'dovich | Caustic formation |
| `disk_exponential` | Exponential disk | Disk stability (Toomre Q) |
| `tidal_stream` | Tidal Plummer | Stream generation |
| `tidal_nfw_host` | Tidal + NFW host | NFW host potential |

### Stability and testing

| Preset | Notes |
|---|---|
| `jeans_instability` | Gravitational instability growth rate |
| `jeans_stability` | Stable mode (should not grow) |
| `debug` | Minimal 4^3 x 4^3 grid for debugging |

## Sweep config

```toml
base_config = "configs/balanced.toml"
output_dir = "output/sweep"

[sweep]
parameters = ["domain.spatial_resolution", "solver.integrator"]

[sweep.values]
"domain.spatial_resolution" = [8, 16, 32]
"solver.integrator" = ["strang", "yoshida"]
```

Runs a batch simulation for every combination in the Cartesian product. Parameter paths use dot notation matching the config structure.

## Convergence config

```toml
base_config = "configs/balanced.toml"
output_dir = "output/convergence"

[convergence]
resolutions = [8, 16, 32, 64]
velocity_scale = true
metrics = ["energy_drift", "mass_drift"]
```

Runs at increasing resolutions and computes convergence rates as `log2(error_N / error_2N)`.

## Supported models

| Model | Config key | Description |
|---|---|---|
| Plummer | `plummer` | Isotropic sphere with analytic DF, f(E) |
| Hernquist | `hernquist` | Galaxy model with closed-form DF |
| King | `king` | Tidally truncated (Poisson-Boltzmann ODE + RK4) |
| NFW | `nfw` | Dark matter halo (numerical Eddington inversion) |
| Zel'dovich | `zeldovich` | Single-mode cosmological pancake |
| Merger | `merger` | Two-body superposition of equilibrium ICs |
| Tidal | `tidal` | Progenitor in external host potential |
| Disk | `disk_exponential` / `disk_stability` | Exponential disk with Shu DF and Toomre Q |
| Uniform perturbation | `uniform_perturbation` | Perturbed Maxwellian (Jeans instability) |
| Custom file | `custom_file` | User-provided 6D .npy array |

## Export formats

Press `e` to open the export menu:

| Key | Format | Description |
|---|---|---|
| `1` | SVG screenshot | Current view as vector graphics |
| `2` | CSV time series | Diagnostics history |
| `3` | JSON time series | Diagnostics history |
| `4` | Parquet time series | Columnar diagnostics |
| `5` | TOML config | Current config |
| `6` | VTK snapshot | ParaView-compatible density |
| `7` | NumPy .npy | Raw density array |
| `8` | Markdown report | Full simulation report |
| `9` | Frame sequence | Animation frames |
| `0` | CSV radial profiles | Radial density/dispersion |
| `a` | Parquet performance | Step timing data |
| `z` | ZIP archive | Everything: config, diagnostics, snapshots, scripts |

## Layout modes

phasma adapts to terminal size:

| Mode | Size | Behavior |
|---|---|---|
| Compact | 40x12 – 79x23 | Single-panel tabs, abbreviated labels |
| Normal | 80x24 – 159x49 | Standard layout |
| Wide | 160x50+ | Three-column panels where applicable |

Panels below minimum size show a "(too small)" placeholder.

## Project structure

```
phasma/
├── Cargo.toml
├── configs/                    # 30 preset TOML configurations
│   ├── balanced.toml
│   ├── default.toml
│   ├── nfw_high_res.toml
│   ├── ...
│   └── debug.toml
└── src/
    ├── main.rs                 # Entry point, mode dispatch
    ├── sim.rs                  # caustic integration, verbose logging, SimState
    ├── config/
    │   ├── mod.rs              # PhasmaConfig schema (serde, all sections)
    │   ├── defaults.rs         # Memory estimation, smart defaults
    │   ├── presets.rs           # Preset save/load
    │   ├── validate.rs          # Config validation
    │   └── history.rs           # Recent config tracking
    ├── runner/
    │   ├── batch.rs            # Headless batch runner
    │   ├── wizard.rs           # Interactive config wizard
    │   ├── sweep.rs            # Parameter sweep
    │   ├── convergence.rs      # Convergence study
    │   ├── compare.rs          # Batch comparison report
    │   ├── regression.rs       # Regression testing
    │   └── monitor.rs          # Filesystem watcher for --monitor/--tail
    ├── data/
    │   ├── live.rs             # Live data provider (ring buffer, scrub history)
    │   ├── playback.rs         # Playback data provider
    │   └── comparison.rs       # Comparison data provider (A/B/diff)
    ├── tui/
    │   ├── app.rs              # Application state machine, global keybindings
    │   ├── cli.rs              # CLI argument definitions (clap)
    │   ├── tabs/               # 10 tab implementations
    │   ├── widgets/            # Reusable widgets (heatmap, colorbar, sparkline table)
    │   ├── status_bar.rs       # Bottom bar (ETA, throughput, RSS memory, rank)
    │   ├── help.rs             # Help overlay (?-key)
    │   ├── export_menu.rs      # 12-item export format selector
    │   ├── command_palette.rs  # Command palette (:)
    │   ├── layout.rs           # Responsive layout (compact/normal/wide)
    │   └── guard.rs            # Terminal size guard
    ├── export/                 # Export format implementations
    ├── colormaps/              # Terminal colormaps (viridis, inferno, plasma, etc.)
    ├── themes.rs               # Color themes (dark, light, solarized, gruvbox)
    ├── annotations.rs          # Time annotations and bookmarks
    ├── notifications.rs        # Desktop notifications on sim events
    └── session.rs              # Session state persistence
```

## Relationship to caustic

phasma is a **consumer** of the caustic library. It provides no solver logic — it constructs a `caustic::Simulation` from user input, runs it on a background thread, and renders live diagnostics.

**`caustic` (lib)** <-- depends on <-- **`phasma` (bin)**

| caustic provides | phasma provides |
|---|---|
| 6D Vlasov-Poisson simulation engine | ratatui TUI with 10 live tabs |
| 7 phase-space representations | TOML config loading with 30 presets |
| 7 Poisson solvers | Real-time density/phase-space heatmaps |
| 6 time integrators | Energy conservation charts and radial profiles |
| 10 IC generators | History scrubbing, playback, comparison |
| LoMaC conservation framework | Batch mode, sweeps, convergence studies |
| Diagnostics and exit conditions | Export (CSV, JSON, NPY, Parquet, VTK, ZIP) |
| rayon parallelism | Verbose logging (`--verbose`) |

To use caustic directly in your own application:

```rust
use caustic::*;

let domain = Domain::builder()
    .spatial_extent(10.0)
    .velocity_extent(3.0)
    .spatial_resolution(16)
    .velocity_resolution(16)
    .t_final(10.0)
    .build()?;

let ic = PlummerIC::new(1.0, 1.0, 1.0);
let snap = sample_on_grid(&ic, &domain);

let mut sim = Simulation::builder()
    .domain(domain)
    .poisson_solver(FftIsolated::new(&domain))
    .advector(SemiLagrangian::new())
    .integrator(YoshidaSplitting::new(1.0))
    .initial_conditions(snap)
    .time_final(10.0)
    .build()?;

while let Ok(None) = sim.step() {
    println!("t={:.4}, E={:.6}", sim.time, sim.diagnostics.history.last().unwrap().total_energy);
}
```

## License

GNU General Public License v3.0. See [LICENSE](LICENSE).

## Citation

```bibtex
@software{phasma,
  title  = {phasma: Terminal interface for the caustic Vlasov-Poisson solver},
  url    = {https://github.com/resonant-jovian/phasma},
  year   = {2026}
}
```
