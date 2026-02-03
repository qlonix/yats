import { useState, useEffect, useMemo } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";

function App() {
  const [mappings, setMappings] = useState({});
  const [isTouched, setIsTouched] = useState(false);
  const [releaseDelay, setReleaseDelay] = useState(100);
  const [showAbout, setShowAbout] = useState(false);
  const [listeningForKey, setListeningForKey] = useState(false);
  const [sortField, setSortField] = useState("key");
  const [sortOrder, setSortOrder] = useState("asc");
  const [isPaused, setIsPaused] = useState(false);
  const [autoStart, setAutoStart] = useState(false);

  useEffect(() => {
    invoke("get_config").then((config) => {
      // Robust loading...
      const rawMappings = config.mappings || {};
      const normalized = {};
      Object.keys(rawMappings).forEach(key => {
        let entry = rawMappings[key];
        if (Array.isArray(entry)) {
          normalized[key] = entry[0] || { type: "MouseClick", value: "Left" };
        } else {
          normalized[key] = entry;
        }
      });
      setMappings(normalized);
      setReleaseDelay(config.release_delay_ms || 200); // Default 200
    });

    invoke("get_paused").then(setIsPaused);

    // Listen for Tray events
    import("@tauri-apps/api/event").then(({ listen }) => {
      listen("pause-status", (event) => {
        setIsPaused(event.payload);
      });
    });

    const interval = setInterval(() => {
      invoke("get_touch_status").then(setIsTouched);
      // invoke("get_paused").then(setIsPaused); // Polling is fallback, event is primary
    }, 500);
    return () => clearInterval(interval);
  }, []);

  const togglePause = () => {
    const newVal = !isPaused;
    setIsPaused(newVal);
    invoke("set_paused", { paused: newVal });
  };

  const toggleAutoStart = (e) => {
    const newVal = e.target.checked;
    setAutoStart(newVal);
    invoke("set_auto_start", { enable: newVal });
  };

  const saveConfig = (newMappings, newDelay) => {
    invoke("set_config", {
      newConfig: {
        mappings: newMappings || mappings,
        release_delay_ms: newDelay !== undefined ? newDelay : releaseDelay
      }
    }).catch(console.error);
  };

  const updateAction = (key, actionType, actionValue) => {
    let finalValue = actionValue;
    if (actionType === "KeyMacro") {
      if (!Array.isArray(actionValue)) finalValue = [];
    } else if (actionType === "MouseClick" || actionType === "MouseDoubleClick") {
      if (typeof actionValue !== "string") finalValue = "Left";
    } else if (actionType === "Window") {
      if (typeof actionValue !== "string") finalValue = "Close";
    } else if (actionType === "MouseScroll") {
      if (typeof actionValue !== "object" || !actionValue) finalValue = { sensitivity: 100 };
    }

    const newMappings = { ...mappings, [key]: { type: actionType, value: finalValue } };
    setMappings(newMappings);
    saveConfig(newMappings);
  };

  const removeMapping = (key) => {
    const newMappings = { ...mappings };
    delete newMappings[key];
    setMappings(newMappings);
    saveConfig(newMappings);
  };

  const formatKeyLabel = (key) => {
    if (!key) return "Unknown";
    if (key.startsWith("Key")) return key.slice(3).toUpperCase();
    if (key.startsWith("Num")) return key.slice(3);
    return key;
  };

  const sortedKeys = useMemo(() => {
    return Object.keys(mappings).sort((keyA, keyB) => {
      const actA = mappings[keyA];
      const actB = mappings[keyB];
      let valA, valB;

      if (sortField === "key") {
        valA = keyA || "";
        valB = keyB || "";
      } else {
        valA = actA?.type || "";
        valB = actB?.type || "";
      }

      const multiplier = sortOrder === "asc" ? 1 : -1;
      return valA.localeCompare(valB) * multiplier;
    });
  }, [mappings, sortField, sortOrder]);

  const toggleSort = (field) => {
    if (sortField === field) {
      setSortOrder(sortOrder === "asc" ? "desc" : "asc");
    } else {
      setSortField(field);
      setSortOrder("asc");
    }
  };

  const normalizeKey = (e) => {
    let key = e.key;
    if (key.length === 1) {
      if (/[a-zA-Z]/.test(key)) return "Key" + key.toUpperCase();
      if (/[0-9]/.test(key)) return "Num" + key;
      return key;
    }
    const map = {
      "Control": "ControlLeft", "Shift": "ShiftLeft", "Alt": "Alt", "Meta": "MetaLeft",
      "Escape": "Escape", "Enter": "Return", "Backspace": "Backspace", "Tab": "Tab",
      " ": "Space", "ArrowUp": "UpArrow", "ArrowDown": "DownArrow",
      "ArrowLeft": "LeftArrow", "ArrowRight": "ArrowRight", "Delete": "Delete"
    };
    return map[key] || key;
  };

  useEffect(() => {
    if (!listeningForKey) return;
    const handleKeyDown = (e) => {
      e.preventDefault();
      const normKey = normalizeKey(e);
      if (!mappings[normKey]) {
        const newMappings = { ...mappings, [normKey]: { type: "MouseClick", value: "Left" } };
        setMappings(newMappings);
        saveConfig(newMappings);
      }
      setListeningForKey(false);
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [listeningForKey, mappings]);

  return (
    <div className="container">
      {showAbout && (
        <div className="modal-overlay" onClick={() => setShowAbout(false)}>
          <div className="modal-content" onClick={(e) => e.stopPropagation()}>
            <h2>About YATS</h2>
            <p><strong>YATS</strong> stands for:</p>
            <p className="yats-full-name">Yet Another Touchpad Shortcut</p>
            <p className="version-info">Version 0.4.7</p>
            <button className="btn-close-modal" onClick={() => setShowAbout(false)}>Close</button>
          </div>
        </div>
      )}

      {listeningForKey && (
        <div className="modal-overlay">
          <div className="modal-content">
            <h2>Recording...</h2>
            <p>Press any key on your keyboard to assign it as a trigger.</p>
            <button className="btn-close-modal" onClick={() => setListeningForKey(false)}>Cancel</button>
          </div>
        </div>
      )}

      <header>
        <div className="title-group">
          <h1>YATS Settings v0.4.8 <span className="about-link" onClick={() => setShowAbout(true)}>?</span></h1>
          <div className="header-controls">
            <label className="header-control-item">
              <input type="checkbox" checked={autoStart} onChange={toggleAutoStart} />
              Run on Startup
            </label>
            <label className="header-control-item delay-control">
              Release Delay: {releaseDelay}ms
              <input
                type="range" min="10" max="1000" step="10"
                value={releaseDelay}
                onChange={(e) => {
                  const val = parseInt(e.target.value);
                  setReleaseDelay(val);
                  saveConfig(null, val);
                }}
              />
            </label>
          </div>
        </div>
        <div className="status-container">
          <div className={`status-badge ${isTouched ? "active" : ""}`}>
            {isTouched ? "TOUCHED" : "IDLE"}
          </div>
        </div>
      </header>

      <div className="mapping-section">
        <table className="mapping-table">
          <thead>
            <tr>
              <th className="key-cell sortable" onClick={() => toggleSort("key")}>
                Trigger Key {sortField === "key" && (sortOrder === "asc" ? "↑" : "↓")}
              </th>
              <th className="type-cell sortable" onClick={() => toggleSort("type")}>
                Action Type {sortField === "type" && (sortOrder === "asc" ? "↑" : "↓")}
              </th>
              <th className="config-cell">Configuration</th>
              <th className="remove-header"></th>
            </tr>
          </thead>
        </table>
        <div className="scroll-area">
          <table className="mapping-table">
            <tbody>
              {sortedKeys.map((key) => {
                const action = mappings[key];
                if (!action) return null;
                return (
                  <tr key={key}>
                    <td className="key-cell">
                      <span className="key-label">{formatKeyLabel(key)}</span>
                    </td>
                    <td className="type-cell">
                      <select value={action.type} onChange={(e) => updateAction(key, e.target.value, action.value)}>
                        <option value="MouseClick">Single Click</option>
                        <option value="MouseDoubleClick">Double Click</option>
                        <option value="MouseScroll">Scroll</option>
                        <option value="KeyMacro">Key Macro</option>
                        <option value="Window">Window Control</option>
                      </select>
                    </td>
                    <td className="config-cell">
                      <div className="config-content">
                        {(action.type === "MouseClick" || action.type === "MouseDoubleClick") && (
                          <select value={action.value} onChange={(e) => updateAction(key, action.type, e.target.value)}>
                            <option value="Left">Left Button</option>
                            <option value="Right">Right Button</option>
                            <option value="Middle">Middle Button</option>
                          </select>
                        )}
                        {action.type === "MouseScroll" && (
                          <div className="scroll-mini-settings row">
                            <div className="scroll-setting-item">
                              <label>Sensitivity: <span style={{ display: "inline-block", width: "32px", textAlign: "right" }}>{(action.value?.sensitivity || 100) / 10}%</span></label>
                              <input
                                type="range" min="1" max="100" step="1"
                                value={(() => {
                                  // Logarithmic Inverse: Value -> Slider (1-100)
                                  // y = A * exp(B*x) -> x = ln(y/A) / B
                                  // Map 10-1000 to 1-100 slider
                                  const minVal = 10;
                                  const maxVal = 1000;
                                  const val = Math.max(minVal, Math.min(action.value?.sensitivity || 100, maxVal));
                                  const minLog = Math.log(minVal);
                                  const maxLog = Math.log(maxVal);
                                  const sliderVal = 1 + (Math.log(val) - minLog) / (maxLog - minLog) * 99;
                                  return sliderVal;
                                })()}
                                onChange={(e) => {
                                  // Logarithmic Forward: Slider -> Value (10-1000)
                                  const sliderVal = parseFloat(e.target.value);
                                  const minVal = 10;
                                  const maxVal = 1000;
                                  const minLog = Math.log(minVal);
                                  const maxLog = Math.log(maxVal);
                                  // value = exp(minLog + scale * (maxLog - minLog))
                                  const scale = (sliderVal - 1) / 99;
                                  const val = Math.exp(minLog + scale * (maxLog - minLog));
                                  // Round to nearest 10 for clean numbers
                                  const cleanVal = Math.round(val / 10) * 10;
                                  updateAction(key, action.type, { ...action.value, sensitivity: cleanVal });
                                }}
                              />
                            </div>
                            <div className="scroll-setting-item checkbox">
                              <label>
                                <input
                                  type="checkbox"
                                  checked={action.value?.invert || false}
                                  onChange={(e) => updateAction(key, action.type, { ...action.value, invert: e.target.checked })}
                                />
                                Invert
                              </label>
                            </div>
                          </div>
                        )}
                        {action.type === "KeyMacro" && (
                          <div className="macro-inline-editor">
                            <div className="macro-sequence-tiny">
                              {(Array.isArray(action.value) ? action.value : []).map((mKey, idx) => (
                                <div key={idx} className="macro-key-capsule-tiny">
                                  <span>{formatKeyLabel(mKey)}</span>
                                  <button onClick={() => {
                                    const next = [...action.value];
                                    next.splice(idx, 1);
                                    updateAction(key, action.type, next);
                                  }}>×</button>
                                </div>
                              ))}
                            </div>
                            <button className="btn-add-macro-tiny" onClick={() => {
                              const k = prompt("Enter key name (e.g. ControlLeft, C, V):");
                              if (k) updateAction(key, action.type, [...(Array.isArray(action.value) ? action.value : []), k]);
                            }}>+ Key</button>
                          </div>
                        )}
                        {action.type === "Window" && (
                          <select value={action.value} onChange={(e) => updateAction(key, action.type, e.target.value)}>
                            <option value="Close">Close Window</option>
                            <option value="Minimize">Minimize</option>
                            <option value="Maximize">Maximize/Restore</option>
                            <option value="Move">Move Window</option>
                          </select>
                        )}
                      </div>
                    </td>
                    <td className="remove-cell">
                      <button className="btn-remove-table" onClick={() => removeMapping(key)}>×</button>
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
        <button className="btn-add full-width" onClick={() => setListeningForKey(true)}>
          + Add New Shortcut Mapping
        </button>
      </div>
    </div>
  );
}

export default App;
