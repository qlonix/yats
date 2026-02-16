### Fixed
- **Linux Scroll Granularity**: Fixed an issue where scroll events were being accumulated redundantly (once in the hook, once in the driver), causing multi-line jumps. Removed the driver-level accumulator to allow 1-to-1 mapping of scroll events.
- **Touchpad Latency**: Reduced the absolute position accumulation threshold (`ACCUM_THRESHOLD`) from 8 to 1 in the Linux touchpad driver. This eliminates the "dead zone" feeling where small initial movements were ignored.
- **Linux Tray Icon**: Fixed the "Pause" menu item on Zorin OS (and other GNOME-based distros) where `CheckMenuItem` was not rendering correctly. Replaced with a dynamic text label ("⏸ 一時停止" / "▶ 機能を再開").

### Changed
- **Default Scroll Parameters**: Updated default scroll settings based on user feedback (Sensitivity: 1, Speed: 5, Natural Scroll: On, Max Speed: 100).
- **UI Improvements**:
  - Moved "Natural Scroll Direction" checkbox to the top of the Scroll Tuning screen and inverted its logic (checked = natural/non-inverted).
  - Capped "Max Scroll Output Speed" slider to 200 (was 3000) for finer control.
  - Hidden "Advanced Settings" button to simplify the UI.
  - Disabled text selection globally via CSS (`user-select: none`).
- **Package Metadata**: Added comprehensive metadata (Homepage, License, Section, Priority) to the generated `.deb` package.
