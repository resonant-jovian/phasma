# phasma

**Terminal interface for the [caustic](https://github.com/resonant-jovian/caustic) Vlasov-Poisson solver.**

[![Crates.io](https://img.shields.io/crates/v/phasma.svg)](https://crates.io/crates/phasma)
[![License: GPL-3.0](https://img.shields.io/badge/License-GPL--3.0-blue.svg)](https://www.gnu.org/licenses/gpl-3.0)

---

phasma is a terminal application built with [ratatui](https://ratatui.rs/) that provides a full interactive workflow for setting up, running, and monitoring caustic simulations — no GUI or web stack required. Everything runs in your terminal over SSH, in tmux, on a headless compute node, wherever you need it.

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
phasma --generate-man > phasma.1 | sudo mv phasma.1 /usr/share/man/man1
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

# Batch mode: no TUI, output saved to disk
phasma --config my_run.toml --batch

# Replay a completed batch run in the TUI
phasma --playback output/run_20260310_143022/

# Side-by-side comparison of two runs
phasma --compare output/run_A/ output/run_B/

# Monitor a batch job in progress
phasma --monitor output/run_20260310_143022/

# Generate a config interactively
phasma --wizard

# Parameter sweep
phasma --sweep sweep.toml

# Convergence study
phasma --convergence convergence.toml

# Regression test (CI-compatible, exits 0 or 1)
phasma --regression-test output/reference_run/

# Compare multiple runs, write Markdown report
phasma --batch-compare output/run_A/ output/run_B/ output/run_C/ --report results.md
```

## CLI reference

| Flag | Argument | Description |
|---|---|---|
| `-c`, `--config` | `PATH` | Path to simulation config file (TOML). Required for `--run`, `--batch`, `--save-preset`. |
| `--run` | — | Start simulation immediately on launch (TUI mode). |
| `--batch` | — | Headless batch mode. Saves diagnostics.csv, JSON snapshots, and metadata.json to a timestamped output directory. Progress printed to stderr. |
| `--playback` | `DIR` | Replay saved snapshots from a batch output directory in the TUI. Supports scrubbing (Left/Right), play/pause (Space). |
| `--compare` | `DIR DIR` | Side-by-side TUI comparison of two batch output directories. Press `c` to cycle Run A / Run B / Difference views. |
| `--sweep` | `TOML` | Parameter sweep. Runs a batch sim for every combination in a Cartesian product of parameter values. See [Sweep config](#sweep-config). |
| `--convergence` | `TOML` | Convergence study. Runs at increasing resolutions and computes convergence rates. See [Convergence config](#convergence-config). |
| `--regression-test` | `DIR` | Re-run a saved config and compare against the reference output. Exits 0 on pass, 1 on fail. |
| `--monitor` | `DIR` | Watch a batch job's output directory and display new snapshots in the TUI as they appear. |
| `--tail` | `DIR` | Like `--monitor` but always auto-advances to the latest snapshot. |
| `--wizard` | — | Interactive guided wizard that prompts for all parameters and writes a TOML config file. |
| `--save-preset` | `NAME` | Save the config from `--config` as a named preset to `~/.config/phasma/presets/NAME.toml`. |
| `--batch-compare` | `DIR DIR [...]` | Generate a Markdown comparison report across 2+ batch output directories. |
| `--report` | `PATH` | Output file for `--batch-compare` report (default: `comparison_report.md`). |
| `--generate-man` | — | Print a roff man page to stdout. |
| `-h`, `--help` | — | Print help. |
| `-V`, `--version` | — | Print version. |

### Batch output directory layout

When running with `--batch`, phasma creates:

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

## Config file reference

phasma configuration uses TOML format. All sections and all fields are optional — sensible defaults are provided for everything. A minimal config can be as short as:

```toml
[model]
type = "plummer"
```

The full config has 9 top-level sections: `[domain]`, `[model]`, `[solver]`, `[time]`, `[output]`, `[exit]`, `[performance]`, `[playback]`, `[appearance]`.

---

### `[domain]` — Simulation domain

| Key | Type | Default | Accepted values | Description |
|---|---|---|---|---|
| `spatial_extent` | float | `10.0` | Any `> 0` | Half-width of the spatial box in each dimension. Domain spans `[-L, L]^3`. |
| `velocity_extent` | float | `5.0` | Any `> 0` | Half-width of the velocity box in each dimension. Domain spans `[-V, V]^3`. |
| `spatial_resolution` | integer | `8` | Any `> 0` | Number of grid cells per spatial dimension. Total spatial cells = N^3. |
| `velocity_resolution` | integer | `8` | Any `> 0` | Number of grid cells per velocity dimension. Total velocity cells = N^3. |
| `boundary` | string | `"periodic\|truncated"` | `"periodic\|truncated"`, `"periodic\|open"`, `"isolated\|truncated"` | Spatial boundary condition \| velocity boundary condition. |
| `coordinates` | string | `"cartesian"` | `"cartesian"` | Coordinate system. |
| `gravitational_constant` | float | `1.0` | Any `> 0` | Value of G in simulation units. |

Memory usage scales as `spatial_resolution^3 * velocity_resolution^3 * 8 bytes`. A 32^3 x 32^3 grid requires ~8 GB.

```toml
[domain]
spatial_extent = 10.0
velocity_extent = 5.0
spatial_resolution = 16
velocity_resolution = 16
boundary = "periodic|truncated"
coordinates = "cartesian"
gravitational_constant = 1.0
```

---

### `[model]` — Initial conditions

| Key | Type | Default | Accepted values | Description |
|---|---|---|---|---|
| `type` | string | `"plummer"` | `"plummer"`, `"hernquist"`, `"king"`, `"nfw"`, `"zeldovich"`, `"merger"`, `"custom_file"` | Initial condition model. |
| `total_mass` | float | `1.0` | Any `> 0` | Total mass of the system. Alias: `mass`. |
| `scale_radius` | float | `1.0` | Any `> 0` | Characteristic scale radius of the model. |

Models with additional parameters require a sub-table:

#### `[model.king]` — King model (tidally truncated)

| Key | Type | Required | Accepted values | Description |
|---|---|---|---|---|
| `w0` | float | yes | Typical `3.0`–`9.0` | Dimensionless central potential. Higher values = more concentrated. W0=6 gives r_t/r_0 ~ 30. |

```toml
[model]
type = "king"
total_mass = 1.0
scale_radius = 1.0

[model.king]
w0 = 6.0
```

#### `[model.nfw]` — NFW dark matter halo

| Key | Type | Required | Accepted values | Description |
|---|---|---|---|---|
| `concentration` | float | yes | Typical `5.0`–`20.0` | Concentration parameter c = r_vir / r_s. Higher = more centrally concentrated. |

```toml
[model]
type = "nfw"
total_mass = 1.0
scale_radius = 1.0

[model.nfw]
concentration = 10.0
```

#### `[model.zeldovich]` — Zel'dovich pancake

| Key | Type | Required | Accepted values | Description |
|---|---|---|---|---|
| `amplitude` | float | yes | Typical `0.1`–`1.0` | Perturbation amplitude. |
| `wave_number` | float | yes | Any `> 0` | Mode wave number. Caustic forms at t = 1 / (amplitude * wave_number). |

```toml
[model]
type = "zeldovich"
total_mass = 1.0
scale_radius = 1.0

[model.zeldovich]
amplitude = 0.3
wave_number = 1.0
```

#### `[model.merger]` — Two-body merger

| Key | Type | Required | Accepted values | Description |
|---|---|---|---|---|
| `separation` | float | yes | Any `> 0` | Initial distance between the two bodies. |
| `mass_ratio` | float | yes | Any `> 0` | Mass ratio m2/m1. Use `1.0` for equal mass. |

```toml
[model]
type = "merger"
total_mass = 2.0
scale_radius = 1.0

[model.merger]
separation = 6.0
mass_ratio = 1.0
```

#### `[model.uniform_perturbation]` — Perturbed uniform background

| Key | Type | Required | Accepted values | Description |
|---|---|---|---|---|
| `mode_m` | integer | yes | Any `> 0` | Perturbation mode number. |
| `amplitude` | float | yes | Any `> 0` | Perturbation amplitude. |

#### `[model.custom_function]` — Custom shared library

| Key | Type | Required | Accepted values | Description |
|---|---|---|---|---|
| `library_path` | string | yes | File path | Path to a shared library (.so / .dylib). |
| `function_name` | string | yes | Symbol name | Name of the function to call. |

#### `[model.custom_file]` — Custom data file

| Key | Type | Required | Accepted values | Description |
|---|---|---|---|---|
| `file_path` | string | yes | File path | Path to a .npy file containing the 6D distribution function. Array shape must match domain resolution. |
| `format` | string | yes | `"npy"` | File format. |

```toml
[model]
type = "custom_file"
total_mass = 1.0
scale_radius = 1.0

[model.custom_file]
file_path = "my_ic.npy"
format = "npy"
```

---

### `[solver]` — Numerical methods

| Key | Type | Default | Accepted values | Description |
|---|---|---|---|---|
| `representation` | string | `"uniform"` | `"uniform"` | Phase-space representation. Uniform 6D grid. |
| `poisson` | string | `"fft_periodic"` | `"fft_periodic"`, `"fft"`, `"fft_isolated"` | Poisson solver. `fft` is an alias for `fft_periodic`. Use `fft_isolated` for non-periodic (Hockney-Eastwood zero-padded). |
| `advection` | string | `"semi_lagrangian"` | `"semi_lagrangian"` | Advection scheme. Semi-Lagrangian with Catmull-Rom interpolation. |
| `integrator` | string | `"strang"` | `"strang"`, `"yoshida"`, `"lie"` | Time integrator. `strang` = 2nd-order symplectic, `yoshida` = 4th-order (7 sub-steps), `lie` = 1st-order. |

```toml
[solver]
representation = "uniform"
poisson = "fft_periodic"
advection = "semi_lagrangian"
integrator = "yoshida"
```

---

### `[time]` — Time stepping

| Key | Type | Default | Accepted values | Description |
|---|---|---|---|---|
| `t_final` | float | `10.0` | Any `> 0` | Simulation end time. |
| `dt_mode` | string | `"adaptive"` | `"adaptive"`, `"fixed"` | Timestep mode. Adaptive uses CFL constraints. Alias: `dt`. |
| `dt_fixed` | float | `0.1` | Any `> 0` | Fixed timestep (only used when `dt_mode = "fixed"`). |
| `cfl_factor` | float | `0.5` | `(0, 1]` | CFL safety factor for adaptive timestep. Lower = more conservative. |
| `dt_min` | float | `1e-6` | Any `> 0` | Minimum allowed timestep (adaptive mode). |
| `dt_max` | float | `1.0` | Any `> 0` | Maximum allowed timestep (adaptive mode). |

```toml
[time]
t_final = 20.0
dt_mode = "adaptive"
cfl_factor = 0.5
dt_min = 1e-6
dt_max = 1.0
```

---

### `[output]` — Output settings

| Key | Type | Default | Accepted values | Description |
|---|---|---|---|---|
| `directory` | string | `"output"` | Any path | Base output directory. Batch mode creates timestamped subdirectories here. |
| `prefix` | string | `"run"` | Any string | Prefix for output subdirectory names (e.g. `run_20260310_143022`). |
| `snapshot_interval` | float | `1.0` | Any `> 0` | Simulation time between snapshot saves. Alias: `interval`. |
| `checkpoint_interval` | float | `10.0` | Any `> 0` | Simulation time between checkpoint saves. |
| `diagnostics_interval` | float | `0.1` | Any `> 0` | Simulation time between diagnostics CSV rows. |
| `format` | string | `"binary"` | `"binary"` | Snapshot format. |

```toml
[output]
directory = "output"
prefix = "run"
snapshot_interval = 1.0
checkpoint_interval = 10.0
diagnostics_interval = 0.1
format = "binary"
```

---

### `[exit]` — Exit / termination conditions

The simulation terminates when **any** enabled condition triggers.

| Key | Type | Default | Accepted values | Description |
|---|---|---|---|---|
| `energy_drift_tolerance` | float | `0.5` | Any `> 0` | Max allowed \|Delta E / E_0\|. Simulation exits if exceeded. Alias: `energy_tolerance`. |
| `mass_drift_tolerance` | float | `0.1` | Any `> 0` | Max allowed \|Delta M / M_0\|. Alias: `mass_threshold`. |
| `virial_equilibrium` | bool | `false` | `true`, `false` | Exit when virial ratio 2T/\|W\| stabilizes within tolerance. |
| `virial_tolerance` | float | `0.05` | Any `> 0` | Tolerance for virial equilibrium detection. |
| `wall_clock_limit` | float | none | Any `> 0` (seconds) | Maximum wall-clock time in seconds. None = no limit. |
| `rank_saturation` | bool | `false` | `true`, `false` | Exit on tensor rank saturation (for tensor representations). |
| `rank_saturation_steps` | integer | `5` | Any `> 0` | Number of consecutive steps at max rank before exit. |
| `cfl_violation` | bool | `true` | `true`, `false` | Exit on CFL condition violation. |
| `steady_state` | bool | `false` | `true`, `false` | Exit when the solution reaches steady state. |
| `steady_state_tolerance` | float | `1e-6` | Any `> 0` | Threshold for steady state detection. |

```toml
[exit]
energy_drift_tolerance = 0.05
mass_drift_tolerance = 0.01
virial_equilibrium = true
virial_tolerance = 0.05
wall_clock_limit = 3600.0
cfl_violation = true
steady_state = false
```

---

### `[performance]` — Performance tuning

| Key | Type | Default | Accepted values | Description |
|---|---|---|---|---|
| `num_threads` | integer | `0` | `0` = all available, or any `> 0` | Number of rayon threads. |
| `memory_budget_gb` | float | `4.0` | Any `> 0` | Memory budget in GB. Validation warns if the grid exceeds this. |
| `rank_budget_warn` | integer | `64` | Any `> 0` | Warn if tensor rank exceeds this value. |
| `simd` | bool | `true` | `true`, `false` | Enable SIMD optimizations. |
| `allocator` | string | `"system"` | `"system"`, `"jemalloc"`, `"mimalloc"` | Memory allocator (requires corresponding cargo feature). |

```toml
[performance]
num_threads = 0
memory_budget_gb = 8.0
simd = true
allocator = "system"
```

---

### `[playback]` — Playback settings

Used when replaying saved snapshots with `--playback`.

| Key | Type | Default | Accepted values | Description |
|---|---|---|---|---|
| `source_directory` | string | none | Directory path | Path to snapshot directory. |
| `source_prefix` | string | none | Any string | Filename prefix filter. |
| `source_format` | string | `"binary"` | `"binary"` | Snapshot format. |
| `fps` | float | `10.0` | Any `> 0` | Playback frames per second. |
| `loop_playback` | bool | `false` | `true`, `false` | Loop back to start when playback reaches the end. |
| `start_time` | float | none | Any `>= 0` | Start playback at this simulation time. |
| `end_time` | float | none | Any `> start_time` | End playback at this simulation time. |

```toml
[playback]
fps = 15.0
loop_playback = true
```

---

### `[appearance]` — TUI appearance

| Key | Type | Default | Accepted values | Description |
|---|---|---|---|---|
| `theme` | string | `"dark"` | `"dark"`, `"light"`, `"solarized"`, `"gruvbox"` | Color theme. |
| `colormap_default` | string | `"viridis"` | `"viridis"`, `"inferno"`, `"plasma"`, `"magma"`, `"grayscale"`, `"cubehelix"`, `"coolwarm"` | Default colormap for density and phase-space heatmaps. |
| `braille_density` | bool | `true` | `true`, `false` | Use braille characters for density rendering. |
| `border_style` | string | `"rounded"` | `"rounded"`, `"plain"`, `"double"` | Widget border style. |
| `square_pixels` | bool | `true` | `true`, `false` | Compensate for non-square terminal cells in heatmaps. |
| `aspect_ratio_mode` | string | `"letterbox"` | `"letterbox"`, `"stretch"` | How to handle aspect ratio mismatch. |
| `cell_aspect_ratio` | float | `0.5` | Any `> 0` | Terminal cell width/height ratio (most terminals ~ 0.5). |
| `min_columns` | integer | `80` | Any `> 0` | Minimum terminal width (columns). |
| `min_rows` | integer | `24` | Any `> 0` | Minimum terminal height (rows). |

```toml
[appearance]
theme = "dark"
colormap_default = "viridis"
braille_density = true
border_style = "rounded"
square_pixels = true
```

---

## Sweep config

The `--sweep` flag takes a TOML file that specifies a parameter sweep:

```toml
base_config = "configs/plummer.toml"
output_dir = "output/sweep"           # default: "output/sweep"

[sweep]
parameters = ["domain.spatial_resolution", "solver.integrator"]

[sweep.values]
"domain.spatial_resolution" = [8, 16, 32]
"solver.integrator" = ["strang", "yoshida"]

[sweep.run]
parallel = 2     # concurrent runs (default: 1)
```

| Key | Type | Required | Default | Description |
|---|---|---|---|---|
| `base_config` | string | yes | — | Path to the base simulation config TOML. |
| `output_dir` | string | no | `"output/sweep"` | Output directory for all sweep runs. |
| `sweep.parameters` | array of strings | yes | — | Dotted config paths to vary (e.g. `"domain.spatial_resolution"`). |
| `sweep.values.<param>` | array | yes | — | Values for each parameter. Cartesian product of all parameter values is generated. |
| `sweep.run.parallel` | integer | no | `1` | Number of concurrent runs. |

Parameter paths use dot notation matching the config structure (e.g. `domain.spatial_resolution`, `solver.integrator`, `time.t_final`, `model.total_mass`).

---

## Convergence config

The `--convergence` flag takes a TOML file:

```toml
base_config = "configs/plummer.toml"
output_dir = "output/convergence"     # default: "output/convergence"

[convergence]
resolutions = [8, 16, 32, 64]
velocity_scale = true                 # set velocity_resolution = spatial_resolution (default: true)
metrics = ["energy_drift", "mass_drift"]  # default: ["energy_drift", "mass_drift"]
```

| Key | Type | Required | Default | Description |
|---|---|---|---|---|
| `base_config` | string | yes | — | Path to the base simulation config TOML. |
| `output_dir` | string | no | `"output/convergence"` | Output directory. |
| `convergence.resolutions` | array of integers | yes | — | List of spatial resolutions to test. |
| `convergence.velocity_scale` | bool | no | `true` | Also set velocity_resolution = spatial_resolution for each run. |
| `convergence.metrics` | array of strings | no | `["energy_drift", "mass_drift"]` | Metrics to compute convergence rates for. |

Convergence rates are computed as `log2(error_N / error_2N)` between consecutive resolution pairs.

---

## Supported models

| Model | Description | Config key |
|---|---|---|
| Plummer | Isotropic sphere with analytic DF | `plummer` |
| Hernquist | Galaxy model with closed-form DF | `hernquist` |
| King | Tidally truncated (Poisson-Boltzmann ODE) | `king` |
| NFW | Dark matter halo (numerical Eddington inversion) | `nfw` |
| Zel'dovich | Single-mode cosmological pancake | `zeldovich` |
| Merger | Two-body superposition of any equilibrium ICs | `merger` |
| Custom file | User-provided 6D array (.npy) | `custom_file` |

## Preset configurations

phasma ships with 8 preset configurations:

| Preset | Model | Grid | Integrator | Use case |
|---|---|---|---|---|
| `speed_priority` | Plummer | 8^3 x 8^3 | Strang | Fast iteration, smoke tests |
| `resolution_priority` | Plummer | 32^3 x 32^3 | Yoshida | High-accuracy production runs |
| `conservation_priority` | Plummer | 16^3 x 16^3 | Yoshida | Conservation law validation |
| `cosmological` | Zel'dovich | 32^3 x 32^3 | Strang | Caustic formation |
| `king_equilibrium` | King (W0=6) | 16^3 x 16^3 | Strang | Tidally truncated equilibrium |
| `nfw_dark_matter` | NFW (c=10) | 16^3 x 16^3 | Yoshida | Dark matter halo |
| `hernquist_galaxy` | Hernquist | 16^3 x 16^3 | Yoshida | Galaxy model |
| `merger` | 2x Plummer | 16^3 x 16^3 | Strang | Two-body interaction |

```bash
phasma --config configs/speed_priority.toml --run
phasma --config configs/cosmological.toml --batch
phasma --save-preset my_plummer --config configs/speed_priority.toml
```

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
- **F5 Energy** — conservation time series: E(t), T(t), W(t), drift Delta E/E_0, mass drift, Casimir drift, entropy
- **F6 Rank** — rank monitoring (placeholder)
- **F7 Profiles** — spherically averaged rho(r), velocity dispersion, mass profile, circular velocity, anisotropy beta(r)
- **F8 Performance** — step timing, adaptive timestep evolution, cumulative wall time, throughput stats
- **F9 Poisson** — Poisson solver detail (placeholder)
- **F10 Settings** — theme and colormap selection

All time-series charts display the full simulation history from t=0 to the current time, with downsampled older data transitioning seamlessly to high-resolution recent data.

### History scrubbing

phasma stores the last 100 simulation snapshots in a ring buffer. Use `Left`/`Right` to scrub backward/forward through history on any visualization tab. Press `Backspace` to jump back to live.

### Playback mode

Use `--playback DIR` to replay a completed batch run in the TUI. Supports play/pause with `Space`, frame stepping with `Left`/`Right`, and all the same visualization tabs as live mode.

### Comparison mode

Use `--compare DIR_A DIR_B` to load two runs side-by-side. Press `c` to cycle between Run A, Run B, and element-wise Difference views on the density and phase-space tabs.

### Monitor / tail

Use `--monitor DIR` to watch a batch job in progress — the TUI updates as new snapshots appear. `--tail DIR` does the same but always auto-advances to the latest snapshot.

## Keyboard controls

| Key | Action |
|---|---|
| `F1` -- `F10` | Switch tabs (Setup, Run, Density, Phase, Energy, Rank, Profiles, Perf, Poisson, Settings) |
| `Tab` / `Shift+Tab` | Next / previous tab |
| `Space` | Pause / resume simulation (global) |
| `Left` / `Right` | Scrub backward / forward through history |
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
| `c` | Cycle colormap / cycle comparison view (in `--compare` mode) |
| `i` | Toggle info bar |

**Energy (F5):**

| Key | Action |
|---|---|
| `t` / `k` / `w` | Toggle traces: total energy / kinetic / potential |
| `d` | Toggle drift view (Delta E/E_0) |
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
    ├── main.rs                 # Entry point, mode dispatch
    ├── sim.rs                  # caustic integration (IC/solver/integrator dispatch)
    ├── config/
    │   ├── mod.rs              # PhasmaConfig schema (serde, all sections)
    │   ├── presets.rs           # Named preset save/load
    │   ├── validate.rs          # Config validation
    │   └── history.rs           # Recent config tracking
    ├── runner/
    │   ├── mod.rs              # RunMetadata, module re-exports
    │   ├── batch.rs            # Headless batch runner with disk output
    │   ├── live.rs             # TUI live runner
    │   ├── wizard.rs           # Interactive config wizard
    │   ├── sweep.rs            # Parameter sweep
    │   ├── convergence.rs      # Convergence study
    │   ├── compare.rs          # Batch comparison report
    │   ├── regression.rs       # Regression testing
    │   └── monitor.rs          # Filesystem watcher for --monitor/--tail
    ├── data/
    │   ├── mod.rs              # DataProvider trait
    │   ├── live.rs             # Live data provider (diagnostics store, scrub history)
    │   ├── playback.rs         # Playback data provider
    │   └── comparison.rs       # Comparison data provider (A/B/diff)
    ├── tui/
    │   ├── app.rs              # Application state machine
    │   ├── cli.rs              # CLI argument definitions (clap)
    │   ├── tabs/               # Tab implementations (setup, run_control, density, ...)
    │   ├── widgets/            # Reusable widgets (heatmap, colorbar, sparkline table)
    │   ├── status_bar.rs       # Bottom status bar (ETA, throughput, memory)
    │   ├── help.rs             # Help overlay
    │   ├── export_menu.rs      # Export format selector
    │   └── ...
    ├── export/                 # Export formats (CSV, JSON, NPY, Parquet, VTK, ZIP)
    ├── colormaps/              # Terminal colormap implementations
    └── themes.rs               # Color themes (dark, light, solarized, gruvbox)
```

## Relationship to caustic

phasma is a **consumer** of the `caustic` library. It provides no solver logic — it constructs a `caustic::Simulation` from user input, runs it on a background thread, and renders the diagnostics that `caustic` produces.

If you want to embed caustic in your own application, script, or pipeline, use the library directly. phasma is for interactive exploration and monitoring long-running jobs from a terminal.

**`caustic` (lib)** <-- depends on <-- **`phasma` (bin)**

| caustic provides | phasma provides |
|---|---|
| Simulation engine | ratatui TUI |
| Phase-space representations | Config loading (TOML presets) |
| Poisson solvers (FFT periodic/isolated) | Real-time density/phase-space heatmaps |
| Advection (semi-Lagrangian) | Energy conservation charts |
| Time integrators (Strang/Yoshida/Lie) | History scrubbing and playback |
| Diagnostics API | Batch mode, sweeps, convergence studies |
| Exit conditions | Export (CSV, JSON, NPY, Parquet, VTK, ZIP) |

## Minimum supported Rust version

phasma targets **stable Rust 1.85+** (edition 2024).

## License

This project is licensed under the [GNU General Public License v3.0](https://www.gnu.org/licenses/gpl-3.0.en.html). See [LICENSE](LICENSE) for details.

## Citation

If you use phasma in academic work, please cite:

```bibtex
@software{phasma,
  title  = {phasma: Terminal interface for the caustic Vlasov-Poisson solver},
  url    = {https://github.com/resonant-jovian/phasma},
  year   = {2026}
}
```
