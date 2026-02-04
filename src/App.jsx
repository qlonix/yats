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
            <p className="version-info">Version 1.0.1</p>
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

      <header>
        <div className="title-group">
          <h1>YATS Settings v1.0.1 <span className="about-link" onClick={() => setShowAbout(true)}>?</span></h1>
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
