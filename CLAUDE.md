# phasma

ratatui terminal UI + batch runner (v0.0.12) for the caustic Vlasov-Poisson solver. Consumes caustic as a path dependency. Provides no solver logic — constructs a `caustic::Simulation` from config, runs it on a background thread, and renders live diagnostics. Rust edition 2024.

Sibling project: **caustic** at `../caustic` (the core solver library). See `../caustic/CLAUDE.md` for solver architecture, traits, physics, and validation suite.

## Commands

Both projects share an identical `dev.sh` script. Run from within `caustic/` or `phasma/`.

```bash
# Preferred — dev.sh (project-aware defaults)
./dev.sh test                        # caustic: --release --test-threads=1; phasma: debug
./dev.sh test --all                  # test both projects
./dev.sh lint                        # clippy + fmt (auto-lints sibling)
./dev.sh build --release             # release build
./dev.sh run -- --config run.toml    # build & run phasma
./dev.sh bench                       # criterion benchmarks (caustic only)
./dev.sh profile flamegraph          # profiling with interactive target picker
./dev.sh doctor                      # check prerequisites, show install commands
./dev.sh info                        # project info, features, cargo profiles

# Direct cargo (when needed)
cargo build                                               # debug
cargo build --release                                     # release (required for validation tests)
cargo test --release -- --test-threads=1                   # caustic validation tests (231+ tests)
cargo test                                                # phasma tests
cargo clippy
cargo fmt
```

## Architecture

**14 execution modes**: interactive TUI (default), `--run`, `--batch`, `--wizard`, `--sweep`, `--convergence`, `--playback`, `--monitor`, `--tail`, `--compare`, `--batch-compare`, `--regression-test`, `--save-preset`, `--generate-man`.

### Module layout — `src/`

```
main.rs             — CLI dispatch (clap, 14 modes)
sim.rs              — caustic bridge: build_from_config(), SimState (67 fields), SimHandle; re-exports caustic::ExitReason
toml.rs             — legacy TOML deserialization (fallback)
config/             — PhasmaConfig (10 sections), defaults, presets (43), validate, history; imports caustic::decimal_serde
data/               — DataProvider trait: LiveDataProvider (ring buffer), PlaybackDataProvider, ComparisonDataProvider, cache
runner/             — batch, live, sweep, convergence, compare, regression, monitor, wizard, helpers (shared drain_to_final, default_concurrency)
export/             — 14 formats: CSV, JSON, Parquet, NPY, VTK, SVG, Animation, Markdown, ZIP, HDF5, ChartSvg, etc.
tui/                — ratatui app, event loop, layout (compact/normal/wide)
tui/tabs/           — 10 tabs: setup, run_control, density, phase_space, energy, rank, profiles, performance, poisson_detail, settings
tui/widgets/        — form_field, scrubber, data_cursor, sparkline_table, zoom (shared crop/scroll helpers)
tui/components/     — home, density_map, exit_tab, prep_tab, run_tab, tab_view
themes.rs, annotations.rs, notifications.rs, colormaps/, session.rs
```

### sim.rs bridge

`build_from_config()` dispatches all caustic types:
- **ICs**: plummer, hernquist, king, nfw, zeldovich, merger, tidal, uniform_perturbation, disk_exponential/disk_stability, custom_file
- **Representations**: uniform, ht, tensor_train, sheet_tracker, spectral, amr, hybrid
- **Poisson**: fft_periodic, fft_isolated, tensor, multigrid, spherical, tree
- **Integrators**: strang, yoshida, lie, unsplit (rk2/3/4), rkei
- **Conservation**: none, lomac
- Uses `_boxed` builder methods for dynamic dispatch. Falls back to legacy `toml.rs` format.
- **Cross-project sharing**: `ExitReason` and `decimal_serde` are defined in caustic, re-exported/imported by phasma. `ExitReason` has serde aliases for backward-compatible deserialization of old variant names.

## TOML Config Format

`PhasmaConfig` has 10 sections:
```toml
[domain]       # spatial_extent, velocity_extent, spatial_resolution, velocity_resolution, boundary, coordinates, gravitational_constant
[model]        # type, total_mass, scale_radius; sub-tables: [model.king], [model.nfw], [model.zeldovich], [model.merger], [model.disk], [model.tidal], [model.custom_file]
[solver]       # representation, poisson, advection, integrator, conservation; sub-tables: [solver.ht], [solver.slar], [solver.lomac], [solver.multigrid], [solver.exponential_sum]
[time]         # t_final, dt_mode (adaptive|fixed), dt_fixed, cfl_factor, dt_min, dt_max
[output]       # directory, prefix, snapshot_interval, checkpoint_interval, diagnostics_interval, format
[exit]         # energy_drift_tolerance, mass_drift_tolerance, virial_equilibrium, wall_clock_limit, steady_state, casimir_drift_tolerance, caustic_formation
[performance]  # num_threads, memory_budget_gb, simd, allocator
[logging]      # level, output
[playback]     # source_directory, fps, loop_playback
[appearance]   # theme (dark|light|solarized|gruvbox), colormap_default, braille_density, border_style, square_pixels
```

**43 preset configs** in `configs/`: debug, plummer (+ 64/128/hires/ht/tt/spectral/lomac/unsplit/yoshida/multigrid/spherical/tensor_poisson/adaptive/bm4/flow_map/ht_poisson/instrumented/lawson/macro_micro/perturbation/positivity/range_separated/rkn6), hernquist, isochrone, king, nfw (+ tree), merger_equal, merger_unequal, zeldovich (+ cosmological), disk_bar, tidal_point, tidal_nfw, jeans_stable, jeans_unstable, fujiwara, mixing, sine_wave_collapse.

## UI

**Tabs** (F1-F10): Setup (F1), Run Control (F2), Density (F3, 2D heatmap projections), Phase-space (F4, all 9 x_i-v_j marginals), Energy (F5, 4-panel conservation), Rank (F6, HT/TT rank evolution), Profiles (F7, radial rho/sigma/M/v_c/beta + Lagrangian radii), Performance (F8, step timing breakdown), Poisson (F9, residual + potential spectrum), Settings (F10, theme/colormap selector).

**Key global keybindings**: F1-F10/Tab switch tabs, Space pause/resume, Left/Right scrub, `?` help, `e` export, `T` theme, `C` colormap, `:` command palette, `a` annotate, `q` quit.

## Current Implementation State

- 10 tabs (F1-F10), responsive layout (compact/normal/wide)
- 14 execution modes, 43 preset configs, 14 export formats
- Data providers: live (ring buffer + scrubbing), playback, comparison
- Smart defaults by model type, comprehensive config validation
- Themes (dark/light/solarized/gruvbox), colormaps (viridis/inferno/plasma/magma/grayscale/cubehelix/coolwarm)
- Mouse support, annotations/bookmarks, command palette, desktop notifications (optional)

**Key dependencies**: ratatui, crossterm, tokio, clap, crossbeam-channel, arrow/parquet, ndarray, zip, chrono, notify-rust, ratatui-plt.
