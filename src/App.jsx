import { useState, useEffect, useMemo, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { DragDropContext, Droppable, Draggable } from "@hello-pangea/dnd";
import "./App.css";

const normalizeKey = (e) => {
  let key = e.key;

  const map = {
    "Control": "ControlLeft", "Shift": "ShiftLeft", "Alt": "Alt", "Meta": "MetaLeft",
    "Escape": "Escape", "Enter": "Return", "Backspace": "Backspace", "Tab": "Tab",
    " ": "Space", "ArrowUp": "UpArrow", "ArrowDown": "DownArrow",
    "ArrowLeft": "LeftArrow", "ArrowRight": "RightArrow", "Delete": "Delete",
    ".": "Dot", ",": "Comma", "/": "Slash", ";": "SemiColon", "'": "Quote",
    "[": "LeftBracket", "]": "RightBracket", "\\": "BackSlash", "-": "Minus", "=": "Equal",
    "`": "BackQuote"
  };

  if (map[key]) return map[key];

  if (key.length === 1) {
    if (/[a-zA-Z]/.test(key)) return "Key" + key.toUpperCase();
    if (/[0-9]/.test(key)) return "Num" + key;
    return key;
  }

  if (key.startsWith("Arrow")) return key.slice(5) + "Arrow";

  return key;
};

const formatKeyLabel = (key) => {
  if (!key) return "Unknown";
  if (key.startsWith("Key")) return key.slice(3).toUpperCase();
  if (key.startsWith("Num")) return key.slice(3);
  return key;
};

const generateId = () => Math.random().toString(36).substr(2, 9);

const MacroRecorderModal = ({ mappingKey, onClose, onSaveMacro, existingSteps }) => {
  // 既存の手順を Stable ID 付きでラップする
  const [tempSteps, setTempSteps] = useState(() => {
    const initial = Array.isArray(existingSteps) ? JSON.parse(JSON.stringify(existingSteps)) : [];
    return initial.map(step => ({ id: generateId(), chord: step }));
  });
  const [currentChord, setCurrentChord] = useState(new Set());
  const stepsEndRef = useRef(null);
  const activeChordRef = useRef(new Set());

  useEffect(() => {
    stepsEndRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [tempSteps, currentChord]);

  useEffect(() => {
    const handleKey = (e) => {
      if (e.repeat) return;
      e.preventDefault();
      const norm = normalizeKey(e);

      if (e.type === 'keydown') {
        activeChordRef.current.add(norm);
        setCurrentChord(new Set(activeChordRef.current));
      } else if (e.type === 'keyup') {
        if (activeChordRef.current.size > 0) {
          const chordArr = Array.from(activeChordRef.current);
          setTempSteps(prev => [...prev, { id: generateId(), chord: chordArr }]);
          activeChordRef.current.clear();
          setCurrentChord(new Set());
        }
      }
    };

    window.addEventListener("keydown", handleKey);
    window.addEventListener("keyup", handleKey);
    return () => {
      window.removeEventListener("keydown", handleKey);
      window.removeEventListener("keyup", handleKey);
    };
  }, []); // 再生成や競合を避けるため依存配列は空にする

  const deleteStep = (idx) => {
    setTempSteps(prev => prev.filter((_, i) => i !== idx));
  };

  const handleSave = () => {
    // 重要: ユーザーがキーを押したまま保存をクリックした場合、現在のコードを確定する
    let finalStepsObj = [...tempSteps];
    if (activeChordRef.current.size > 0) {
      finalStepsObj.push({ id: generateId(), chord: Array.from(activeChordRef.current) });
    }
    // 空の手順を除外し、生の配列に戻す
    const finalSteps = finalStepsObj
      .filter(s => s.chord && s.chord.length > 0)
      .map(s => s.chord);

    console.log("Saving macro for", mappingKey, ":", finalSteps);
    onSaveMacro(finalSteps);
  };

  const handleDragEnd = (result) => {
    if (!result.destination) return;

    const sourceIdx = result.source.index;
    const targetIdx = result.destination.index;

    if (sourceIdx === targetIdx) return;

    const newSteps = [...tempSteps];
    const [reorderedItem] = newSteps.splice(sourceIdx, 1);
    newSteps.splice(targetIdx, 0, reorderedItem);

    setTempSteps(newSteps);
  };

  const moveStep = (idx, direction) => {
    const newSteps = [...tempSteps];
    if (direction === -1 && idx > 0) {
      // Move Up
      const item = newSteps[idx];
      newSteps.splice(idx, 1);
      newSteps.splice(idx - 1, 0, item);
      setTempSteps(newSteps);
    } else if (direction === 1 && idx < newSteps.length - 1) {
      // Move Down
      const item = newSteps[idx];
      newSteps.splice(idx, 1);
      newSteps.splice(idx + 1, 0, item);
      setTempSteps(newSteps);
    }
  };

  return (
    <div className="modal-overlay" onClick={onClose}>
      <div className="modal-content macro-recorder" onClick={(e) => e.stopPropagation()}>
        <h2>Recording Macro for {formatKeyLabel(mappingKey)}</h2>
        <div className="macro-steps-preview">
          <DragDropContext onDragEnd={handleDragEnd}>
            <Droppable droppableId="macro-steps">
              {(provided) => (
                <div
                  {...provided.droppableProps}
                  ref={provided.innerRef}
                  style={{ display: 'flex', flexDirection: 'column', gap: '10px' }}
                >
                  {tempSteps.map((stepObj, idx) => (
                    <Draggable key={stepObj.id} draggableId={stepObj.id} index={idx}>
                      {(provided, snapshot) => (
                        <div
                          ref={provided.innerRef}
                          {...provided.draggableProps}
                          className={`macro-step-item ${snapshot.isDragging ? 'dragging' : ''}`}
                          style={{
                            ...provided.draggableProps.style,
                          }}
                        >
                          <div
                            className="drag-handle"
                            {...provided.dragHandleProps}
                            title="Drag to reorder"
                          >
                            ⋮⋮
                          </div>
                          <span className="step-idx">{idx + 1}.</span>
                          <div className="step-keys">
                            {stepObj.chord.map(k => <span key={k} className="key-cap">{formatKeyLabel(k)}</span>)}
                          </div>
                          <div style={{ flex: 1 }}></div>
                          <div className="step-actions">
                            <button
                              className="btn-move-step"
                              onClick={() => moveStep(idx, -1)}
                              disabled={idx === 0}
                              title="Move Up"
                            >
                              ↑
                            </button>
                            <button
                              className="btn-move-step"
                              onClick={() => moveStep(idx, 1)}
                              disabled={idx === tempSteps.length - 1}
                              title="Move Down"
                            >
                              ↓
                            </button>
                            <button className="btn-delete-step" onClick={() => deleteStep(idx)} title="Delete step">×</button>
                          </div>
                        </div>
                      )}
                    </Draggable>
                  ))}
                  {provided.placeholder}
                </div>
              )}
            </Droppable>
          </DragDropContext>
          {currentChord.size > 0 && (
            <div className="macro-step-item current">
              <span className="step-idx">...</span>
              <div className="step-keys">
                {Array.from(currentChord).map(k => <span key={k} className="key-cap active">{formatKeyLabel(k)}</span>)}
              </div>
            </div>
          )}
          <div ref={stepsEndRef} />
        </div>
        <div className="recorder-controls">
          <p className="hint">Actually press keys on your keyboard to record steps.</p>
          <div className="modal-actions">
            <button className="btn-save" onClick={handleSave}>Save Macro</button>
            <button className="btn-clear" onClick={() => setTempSteps([])}>Clear All</button>
            <button className="btn-close-modal" onClick={onClose}>Cancel</button>
          </div>
        </div>
      </div>
    </div>
  );
};

const YatsLogo = () => (
  <svg width="40" height="40" viewBox="0 0 1024 1024" className="yats-logo-svg">
    <defs>
      <linearGradient id="logoGrad" x1="0%" y1="0%" x2="100%" y2="100%">
        <stop offset="0%" style={{ stopColor: "#00d2ff", stopOpacity: 1 }} />
        <stop offset="100%" style={{ stopColor: "#3a7bd5", stopOpacity: 1 }} />
      </linearGradient>
    </defs>
    <rect x="120" y="200" width="784" height="624" rx="80" fill="url(#logoGrad)" />
    <rect x="220" y="300" width="180" height="180" rx="40" fill="white" />
    <rect x="422" y="300" width="180" height="180" rx="40" fill="white" />
    <rect x="624" y="300" width="180" height="180" rx="40" fill="white" />
    <circle cx="512" cy="650" r="60" fill="none" stroke="white" strokeWidth="40" />
    <circle cx="512" cy="650" r="20" fill="white" />
  </svg>
);

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
  const [recordingMacroKey, setRecordingMacroKey] = useState(null);
  const [showScrollSettings, setShowScrollSettings] = useState(false);
  const [scrollSensitivity, setScrollSensitivity] = useState(100);
  const [scrollSpeed, setScrollSpeed] = useState(100);
  const [scrollInvert, setScrollInvert] = useState(false);

  useEffect(() => {
    invoke("get_config").then((config) => {
      // Robust loading...
      const rawMappings = config.mappings || {};
      const normalized = {};
      Object.keys(rawMappings).forEach(key => {
        let entry = rawMappings[key];
        if (Array.isArray(entry)) {
          entry = entry[0] || { type: "MouseClick", value: "Left" };
        }

        // Migration: KeyMacro normalization for v0.4.8-1
        if (entry.type === "KeyMacro") {
          // If it's a flat array (old format), wrap each key in its own step-array
          if (entry.value && entry.value.length > 0 && typeof entry.value[0] === 'string') {
            entry.value = entry.value.map(k => [k]);
          } else if (!entry.value) {
            entry.value = [];
          }
        }

        normalized[key] = entry;
      });
      setMappings(normalized);
      setReleaseDelay(config.release_delay_ms || 200);
      setScrollSensitivity(config.scroll_sensitivity || 100);
      setScrollSpeed(config.scroll_speed || 100);
      setScrollInvert(config.scroll_invert || false);
    });

    invoke("get_paused").then(setIsPaused);

    // Listen for Tray events
    import("@tauri-apps/api/event").then(({ listen }) => {
      listen("pause-status", (event) => {
        setIsPaused(event.payload);
      });
    });

    invoke("get_startup_status_cmd").then(setAutoStart);

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
    invoke("set_startup_cmd", { enabled: newVal });
  };

  const saveConfig = (newMappings, newDelay, newSens, newInvert, newSpeed) => {
    invoke("set_config", {
      newConfig: {
        mappings: newMappings || mappings,
        release_delay_ms: (newDelay !== null && newDelay !== undefined) ? newDelay : releaseDelay,
        scroll_sensitivity: (newSens !== null && newSens !== undefined) ? newSens : scrollSensitivity,
        scroll_speed: (newSpeed !== null && newSpeed !== undefined) ? newSpeed : scrollSpeed,
        scroll_invert: (newInvert !== null && newInvert !== undefined) ? newInvert : scrollInvert,
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
            <p className="version-info">Version 1.1.0</p>
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

      {recordingMacroKey && (
        <MacroRecorderModal
          mappingKey={recordingMacroKey}
          existingSteps={mappings[recordingMacroKey]?.value}
          onClose={() => setRecordingMacroKey(null)}
          onSaveMacro={(steps) => {
            updateAction(recordingMacroKey, "KeyMacro", steps);
            setRecordingMacroKey(null);
          }}
        />
      )}

      {showScrollSettings && (
        <div className="modal-overlay" onClick={() => setShowScrollSettings(false)}>
          <div className="modal-content" onClick={(e) => e.stopPropagation()}>
            <h2>Global Scroll Settings</h2>
            <div className="modal-body" style={{ padding: "20px 0" }}>
              <div style={{ marginBottom: "20px" }}>
                <label style={{ display: "block", marginBottom: "8px", fontSize: "0.9rem", color: "#888" }}>
                  Sensitivity: <span className="value-display" style={{ fontSize: "1.1rem" }}>{scrollSensitivity}</span>%
                </label>
                <input
                  type="range" min="0" max="100" step="1"
                  style={{ width: "100%", height: "6px", accentColor: "#00d2ff" }}
                  // v2.7.0: UI uses logarithmic mapping for better control at lower ranges. 
                  // pos = 100 * log10(val) / 2
                  value={Math.round(100 * Math.log10(scrollSensitivity) / 2)}
                  onChange={(e) => {
                    const pos = parseInt(e.target.value);
                    // val = 100^(pos/100)
                    const val = Math.round(Math.pow(100, pos / 100));
                    setScrollSensitivity(val);
                    saveConfig(null, null, val);
                  }}
                />
                <p style={{ fontSize: "0.75rem", color: "#555", marginTop: "8px" }}>
                  * This setting affects ALL scroll shortcuts. Control is finer at lower ranges.
                </p>
              </div>
              <label className="header-control-item" style={{ fontSize: "1rem", color: "#e0e0e0", cursor: "pointer" }}>
                <input
                  type="checkbox"
                  style={{ width: "18px", height: "18px" }}
                  checked={scrollInvert}
                  onChange={(e) => {
                    const val = e.target.checked;
                    setScrollInvert(val);
                    saveConfig(null, null, null, val);
                  }}
                />
                Invert Scroll Direction
              </label>
              <div style={{ marginTop: "20px" }}>
                <label style={{ display: "block", marginBottom: "8px", fontSize: "0.9rem", color: "#888" }}>
                  Scroll Speed: <span className="value-display" style={{ fontSize: "1.1rem" }}>{scrollSpeed}</span>%
                </label>
                <input
                  type="range" min="10" max="500" step="10"
                  style={{ width: "100%", height: "6px", accentColor: "#3a7bd5" }}
                  value={scrollSpeed}
                  onChange={(e) => {
                    const val = parseInt(e.target.value);
                    setScrollSpeed(val);
                    saveConfig(null, null, null, null, val);
                  }}
                />
                <p style={{ fontSize: "0.75rem", color: "#555", marginTop: "8px" }}>
                  * Controls how fast the page scrolls. 100% = default, lower = slower, higher = faster.
                </p>
              </div>
            </div>
            <button className="btn-close-modal" onClick={() => setShowScrollSettings(false)}>Close</button>
          </div>
        </div>
      )}

      <header>
        <div className="title-group">
          <YatsLogo />
          <h1>YATS Settings <span className="about-link" onClick={() => setShowAbout(true)}>?</span></h1>
          <div className="header-controls">
            <label className="header-control-label">General:</label>
            <div className="header-controls-row">
              <label className="header-control-item">
                <input type="checkbox" checked={autoStart} onChange={toggleAutoStart} />
                Run on Startup
              </label>
              <label className="header-control-item delay-control">
                Release Delay: <span className="value-display">{releaseDelay}</span>ms
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
                          <button className="btn-global-config" onClick={() => setShowScrollSettings(true)}>
                            Configure Global Scroll...
                          </button>
                        )}
                        {action.type === "KeyMacro" && (
                          <div className="macro-inline-editor">
                            <div className="macro-sequence-tiny">
                              {(Array.isArray(action.value) ? action.value : []).map((step, idx) => (
                                <div key={idx} className="macro-step-capsule">
                                  {Array.isArray(step) ? step.map(mk => <span key={mk}>{formatKeyLabel(mk)}</span>).reduce((acc, x) => acc === null ? [x] : [acc, <span key={`plus-${idx}`} className="spacer">+</span>, x], null) : formatKeyLabel(step)}
                                </div>
                              ))}
                            </div>
                            <button className="btn-add-macro-tiny" onClick={() => setRecordingMacroKey(key)}>Record</button>
                          </div>
                        )}
                        {action.type === "Window" && (
                          <select value={action.value} onChange={(e) => updateAction(key, action.type, e.target.value)}>
                            <option value="Close">Close Window</option>
                            <option value="Minimize">Minimize</option>
                            <option value="Maximize">Maximize/Restore</option>
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
