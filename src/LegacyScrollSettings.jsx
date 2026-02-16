import React, { useState } from "react";

const LegacyScrollSettings = ({
    visible,
    onClose,
    scrollSensitivity,
    setScrollSensitivity,
    saveConfigWithLatest,
    scrollSpeed,
    setScrollSpeed,
    linuxMinSpeed,
    setLinuxMinSpeed,
    scrollInvert,
    setScrollInvert,
    linuxMinDistance,
    linuxMinScrollSpeed,
    linuxMaxScrollSpeed,
    linuxScrollCurve
}) => {
    const [exportMsg, setExportMsg] = useState("");

    if (!visible) return null;

    const handleExport = () => {
        const params = {
            scroll_sensitivity: scrollSensitivity,
            scroll_speed: scrollSpeed,
            scroll_invert: scrollInvert,
            linux_min_distance: linuxMinDistance,
            linux_min_speed: linuxMinSpeed,
            linux_min_scroll_speed: linuxMinScrollSpeed,
            linux_max_scroll_speed: linuxMaxScrollSpeed,
            linux_scroll_curve: linuxScrollCurve
        };
        const json = JSON.stringify(params, null, 2);
        navigator.clipboard.writeText(json).then(() => {
            setExportMsg("Copied to clipboard!");
            setTimeout(() => setExportMsg(""), 2000);
        }).catch(() => {
            setExportMsg("Copy failed");
            setTimeout(() => setExportMsg(""), 2000);
        });
    };

    return (
        <div className="modal-overlay" onClick={onClose}>
            <div className="modal-content legacy-scroll-settings" onClick={(e) => e.stopPropagation()}>
                <h2>Legacy Scroll Settings</h2>
                <div className="modal-body" style={{ padding: "20px 0" }}>

                    {/* Scroll Sensitivity (Logarithmic) */}
                    <div style={{ marginBottom: "20px" }}>
                        <label style={{ display: "block", marginBottom: "8px", fontSize: "0.9rem", color: "#888" }}>
                            Sensitivity: <span className="value-display" style={{ fontSize: "1.1rem" }}>{scrollSensitivity}</span>%
                        </label>
                        <input
                            type="range" min="0" max="100" step="1"
                            style={{ width: "100%", height: "6px", accentColor: "#00d2ff" }}
                            // pos = 100 * log10(val) / 2
                            value={scrollSensitivity > 0 ? Math.round(100 * Math.log10(scrollSensitivity) / 2) : 0}
                            onChange={(e) => {
                                const pos = parseInt(e.target.value);
                                // val = 100^(pos/100)
                                let val = Math.round(Math.pow(100, pos / 100));
                                if (val < 1) val = 1;
                                setScrollSensitivity(val);
                                saveConfigWithLatest({ scrollSensitivity: val });
                            }}
                        />
                        <p style={{ fontSize: "0.75rem", color: "#555", marginTop: "8px" }}>
                            * Logarithmic scale: Control is finer at lower ranges. (1% - 100%)
                        </p>
                    </div>

                    {/* Scroll Speed */}
                    <div style={{ marginTop: "20px" }}>
                        <label style={{ display: "block", marginBottom: "8px", fontSize: "0.9rem", color: "#888" }}>
                            Scroll Speed: <span className="value-display" style={{ fontSize: "1.1rem" }}>{Math.round(scrollSpeed * 2)}</span>%
                        </label>
                        <input
                            type="range" min="5" max="50" step="1"
                            style={{ width: "100%", height: "6px", accentColor: "#00d2ff" }}
                            value={scrollSpeed}
                            onChange={(e) => {
                                const val = parseInt(e.target.value);
                                setScrollSpeed(val);
                                saveConfigWithLatest({ scrollSpeed: val });
                            }}
                        />
                        <p style={{ fontSize: "0.75rem", color: "#555", marginTop: "8px" }}>
                            * Base scroll speed multiplier. (10% - 100%)
                        </p>
                    </div>

                    {/* Linux Min Mouse Speed */}
                    <div style={{ marginTop: "20px" }}>
                        <label style={{ display: "block", marginBottom: "8px", fontSize: "0.9rem", color: "#888" }}>
                            Min Mouse Speed (Linux): <span className="value-display" style={{ fontSize: "1.1rem" }}>{linuxMinSpeed}</span> px/s
                        </label>
                        <input
                            type="range" min="0" max="500" step="5"
                            style={{ width: "100%", height: "6px", accentColor: "#00d2ff" }}
                            value={linuxMinSpeed}
                            onChange={(e) => {
                                const val = parseFloat(e.target.value);
                                setLinuxMinSpeed(val);
                                saveConfigWithLatest({ linuxMinSpeed: val });
                            }}
                        />
                        <p style={{ fontSize: "0.75rem", color: "#555", marginTop: "8px" }}>
                            * Minimum physical mouse speed required to trigger scroll.
                        </p>
                    </div>

                    {/* Invert Scroll */}
                    <label className="header-control-item" style={{ fontSize: "1rem", color: "#e0e0e0", cursor: "pointer", marginTop: "24px", display: "flex", alignItems: "center" }}>
                        <input
                            type="checkbox"
                            style={{ width: "18px", height: "18px", marginRight: "10px" }}
                            checked={scrollInvert}
                            onChange={(e) => {
                                const val = e.target.checked;
                                setScrollInvert(val);
                                saveConfigWithLatest({ scrollInvert: val });
                            }}
                        />
                        Invert Scroll Direction
                    </label>

                </div>
                <div className="modal-footer" style={{ display: "flex", gap: "10px", justifyContent: "space-between", alignItems: "center" }}>
                    <button
                        className="btn-close-modal"
                        style={{ background: "#2a5a3a", fontSize: "0.8rem", padding: "6px 14px" }}
                        onClick={handleExport}
                    >
                        📋 Export Params
                    </button>
                    {exportMsg && <span style={{ fontSize: "0.8rem", color: "#0f0" }}>{exportMsg}</span>}
                    <button className="btn-close-modal" onClick={onClose}>Close</button>
                </div>
            </div>
        </div>
    );
};

export default LegacyScrollSettings;
