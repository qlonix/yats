import { useState, useEffect, useMemo, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { DragDropContext, Droppable, Draggable } from "@hello-pangea/dnd";
import LegacyScrollSettings from "./LegacyScrollSettings";
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

const CurveEditor = ({ points, onChange, maxY = 2000 }) => {
  const [draggingIdx, setDraggingIdx] = useState(null);
  const svgRef = useRef(null);
  const width = 480;
  const height = 300;
  const margin = 40;

  // X is fixed to 2000 for mouse speed, Y is dynamic
  const maxX = 2000;

  const toSvgX = (x) => margin + (x / maxX) * (width - 2 * margin);
  const toSvgY = (y) => height - margin - (y / maxY) * (height - 2 * margin);
  const fromSvgX = (sx) => ((sx - margin) / (width - 2 * margin)) * maxX;
  const fromSvgY = (sy) => ((height - margin - sy) / (height - 2 * margin)) * maxY;

  const sortedPoints = [...points].sort((a, b) => a[0] - b[0]);
  const n = sortedPoints.length;

  // Monotone Cubic Spline implementation for preview
  const getSplinePoints = () => {
    if (n < 2) return [];

    const x = sortedPoints.map(p => p[0]);
    const y = sortedPoints.map(p => p[1]);

    const d = [];
    for (let i = 0; i < n - 1; i++) {
      d.push((y[i + 1] - y[i]) / (x[i + 1] - x[i]));
    }

    const m = new Array(n).fill(0);
    m[0] = d[0];
    for (let i = 1; i < n - 1; i++) {
      m[i] = (d[i - 1] + d[i]) / 2;
    }
    m[n - 1] = d[n - 2];

    for (let i = 0; i < n - 1; i++) {
      if (d[i] === 0) {
        m[i] = 0;
        m[i + 1] = 0;
      } else {
        const a = m[i] / d[i];
        const b = m[i + 1] / d[i];
        const h = Math.hypot(a, b);
        if (h > 3) {
          const t = 3 / h;
          m[i] = t * a * d[i];
          m[i + 1] = t * b * d[i];
        }
      }
    }

    const pathPoints = [];
    const steps = 60;
    for (let i = 0; i < n - 1; i++) {
      const h = x[i + 1] - x[i];
      if (h <= 0) continue;
      for (let j = 0; j <= steps; j++) {
        const stepX = x[i] + (h * j) / steps;
        const t = (stepX - x[i]) / h;
        const val = y[i] * (1 + 2 * t) * (1 - t) ** 2
          + h * m[i] * t * (1 - t) ** 2
          + y[i + 1] * t ** 2 * (3 - 2 * t)
          + h * m[i + 1] * t ** 2 * (t - 1);
        pathPoints.push([stepX, val]);
      }
    }
    return pathPoints;
  };

  const handleMouseDown = (e, idx) => {
    e.stopPropagation();
    if (idx !== null) {
      setDraggingIdx(idx);
    }
  };

  const handleMouseMove = (e) => {
    if (draggingIdx === null) return;
    const rect = svgRef.current.getBoundingClientRect();
    const sx = e.clientX - rect.left;
    const sy = e.clientY - rect.top;

    let x = fromSvgX(sx);
    let y = fromSvgY(sy);

    const newPoints = [...sortedPoints];
    x = Math.max(0, Math.min(maxX, x));
    y = Math.max(0, Math.min(maxY, y));

    // Constraint: don't cross neighbors
    if (draggingIdx > 0) x = Math.max(newPoints[draggingIdx - 1][0] + 5, x);
    if (draggingIdx < n - 1) x = Math.min(newPoints[draggingIdx + 1][0] - 5, x);

    newPoints[draggingIdx] = [x, y];
    onChange(newPoints);
  };

  const handleMouseUp = () => {
    setDraggingIdx(null);
  };

  const handleClickLine = (e) => {
    if (draggingIdx !== null) return;
    const rect = svgRef.current.getBoundingClientRect();
    const sx = e.clientX - rect.left;
    const sy = e.clientY - rect.top;

    if (sx < margin || sx > width - margin || sy < margin || sy > height - margin) return;

    let x = fromSvgX(sx);
    let y = fromSvgY(sy);

    x = Math.max(0, Math.min(maxX, x));
    y = Math.max(0, Math.min(maxY, y));

    if (sortedPoints.some(p => Math.abs(toSvgX(p[0]) - sx) < 15)) return;

    const newPoints = [...sortedPoints, [x, y]];
    onChange(newPoints);
  };

  const handleContextMenu = (e, idx) => {
    e.preventDefault();
    if (idx !== null && sortedPoints.length > 2) {
      const newPoints = sortedPoints.filter((_, i) => i !== idx);
      onChange(newPoints);
    }
  };

  const splineData = getSplinePoints();
  const pathData = splineData.length > 0
    ? `M ${toSvgX(splineData[0][0])} ${toSvgY(splineData[0][1])} ` +
    splineData.slice(1).map(p => `L ${toSvgX(p[0])} ${toSvgY(p[1])}`).join(' ')
    : "";

  return (
    <div className="curve-editor-wrapper" onMouseUp={handleMouseUp} onMouseLeave={handleMouseUp}>
      <svg
        ref={svgRef}
        width={width}
        height={height}
        onMouseMove={handleMouseMove}
        onClick={handleClickLine}
        style={{ background: "rgba(0,0,0,0.3)", borderRadius: "12px", cursor: draggingIdx !== null ? "grabbing" : "crosshair" }}
      >
        {/* Grid lines */}
        {[0, 0.25, 0.5, 0.75, 1].map(v => (
          <g key={v}>
            <line x1={margin} y1={toSvgY(v * maxY)} x2={width - margin} y2={toSvgY(v * maxY)} stroke="rgba(255,255,255,0.05)" strokeWidth="1" />
            <line x1={toSvgX(v * maxX)} y1={margin} x2={toSvgX(v * maxX)} y2={height - margin} stroke="rgba(255,255,255,0.05)" strokeWidth="1" />
            <text x={margin - 8} y={toSvgY(v * maxY)} fill="#666" fontSize="10" textAnchor="end" alignmentBaseline="middle">{Math.round(v * maxY)}</text>
            <text x={toSvgX(v * maxX)} y={height - margin + 18} fill="#666" fontSize="10" textAnchor="middle">{Math.round(v * maxX)}</text>
          </g>
        ))}

        <text x={width / 2} y={height - 8} fill="#888" fontSize="11" textAnchor="middle">Input Mouse Speed (px/s)</text>
        <text x={12} y={height / 2} fill="#888" fontSize="11" transform={`rotate(-90, 12, ${height / 2})`} textAnchor="middle">Scroll Output</text>

        <path d={pathData} fill="none" stroke="url(#curveGrad)" strokeWidth="3" strokeLinejoin="round" />
        <defs>
          <linearGradient id="curveGrad" x1="0%" y1="0%" x2="100%" y2="0%">
            <stop offset="0%" style={{ stopColor: "#00d2ff" }} />
            <stop offset="100%" style={{ stopColor: "#3a7bd5" }} />
          </linearGradient>

        </defs>

        {sortedPoints.map((p, i) => (
          <circle
            key={i}
            cx={toSvgX(p[0])}
            cy={toSvgY(p[1])}
            r={draggingIdx === i ? 8 : 6}
            fill={draggingIdx === i ? "#fff" : "#00d2ff"}
            stroke="rgba(255,255,255,0.2)"
            strokeWidth="2"
            onMouseDown={(e) => handleMouseDown(e, i)}
            onContextMenu={(e) => handleContextMenu(e, i)}
            style={{ cursor: "pointer", transition: "r 0.1s" }}
          />
        ))}
      </svg>
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
  const [showScrollTuning, setShowScrollTuning] = useState(false);
  const [scrollInvert, setScrollInvert] = useState(true);
  const [scrollSensitivity, setScrollSensitivity] = useState(1);
  const [scrollSpeed, setScrollSpeed] = useState(5);
  const [linuxMinDistance, setLinuxMinDistance] = useState(0);
  const [linuxMinSpeed, setLinuxMinSpeed] = useState(0.0);
  const [linuxMinScrollSpeed, setLinuxMinScrollSpeed] = useState(1);
  const [linuxMaxScrollSpeed, setLinuxMaxScrollSpeed] = useState(100);
  const [linuxScrollCurve, setLinuxScrollCurve] = useState([
    [0, 1.6335227], [1305, 3.90625], [1730, 21.448864], [2000, 79.360794]
  ]);
  const [showLegacySettings, setShowLegacySettings] = useState(false);

  // Centralized robust save function
  const saveConfigWithLatest = async (overrides = {}) => {
    try {
      // Construction from latest state + overrides
      // This avoids the "get_config then modify" race condition.
      const updated = {
        mappings: overrides.mappings !== undefined ? overrides.mappings : mappings,
        release_delay_ms: overrides.releaseDelay !== undefined ? parseInt(overrides.releaseDelay) : releaseDelay,
        scroll_sensitivity: overrides.scrollSensitivity !== undefined ? parseInt(overrides.scrollSensitivity) : scrollSensitivity,
        scroll_speed: overrides.scrollSpeed !== undefined ? parseInt(overrides.scrollSpeed) : scrollSpeed,
        scroll_invert: overrides.scrollInvert !== undefined ? overrides.scrollInvert : scrollInvert,
        linux_min_distance: overrides.linuxMinDistance !== undefined ? overrides.linuxMinDistance : linuxMinDistance,
        linux_min_speed: overrides.linuxMinSpeed !== undefined ? parseFloat(overrides.linuxMinSpeed) : linuxMinSpeed,
        linux_min_scroll_speed: overrides.linuxMinScrollSpeed !== undefined ? overrides.linuxMinScrollSpeed : linuxMinScrollSpeed,
        linux_max_scroll_speed: overrides.linuxMaxScrollSpeed !== undefined ? overrides.linuxMaxScrollSpeed : linuxMaxScrollSpeed,
        linux_use_scroll_curve: true, // Fixed as baseline
        linux_scroll_curve: overrides.linuxScrollCurve !== undefined ? overrides.linuxScrollCurve : linuxScrollCurve,
      };

      await invoke("set_config", { newConfig: updated });
    } catch (err) {
      console.error("Failed to save config:", err);
    }
  };

  const loadConfig = async () => {
    try {
      const config = await invoke("get_config");
      const rawMappings = config.mappings || {};
      const normalized = {};
      Object.keys(rawMappings).forEach(key => {
        let entry = rawMappings[key];
        if (Array.isArray(entry)) {
          entry = entry[0] || { type: "MouseClick", value: "Left" };
        }
        if (entry.type === "KeyMacro") {
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
      setScrollSensitivity(config.scroll_sensitivity ?? 1);
      setScrollSpeed(config.scroll_speed ?? 5);
      setScrollInvert(config.scroll_invert ?? true);
      setLinuxMinDistance(config.linux_min_distance ?? 0);
      setLinuxMinSpeed(config.linux_min_speed ?? 0.0);
      setLinuxMinScrollSpeed(config.linux_min_scroll_speed ?? 1);
      setLinuxMaxScrollSpeed(config.linux_max_scroll_speed ?? 100);
      setLinuxScrollCurve(config.linux_scroll_curve && config.linux_scroll_curve.length > 0 ? config.linux_scroll_curve : [
        [0, 1.6335227], [1305, 3.90625], [1730, 21.448864], [2000, 79.360794]
      ]);
    } catch (err) {
      console.error("Failed to load config:", err);
    }
  };

  useEffect(() => {
    loadConfig();
    invoke("get_paused_cmd").then(setIsPaused);
    import("@tauri-apps/api/event").then(({ listen }) => {
      listen("pause-status", (event) => setIsPaused(event.payload));
    });
    invoke("get_startup_status_cmd").then(setAutoStart);

    const interval = setInterval(() => {
      invoke("get_touch_status").then(setIsTouched);
    }, 500);
    return () => clearInterval(interval);
  }, []);

  const togglePause = () => {
    const newVal = !isPaused;
    setIsPaused(newVal);
    invoke("set_paused_cmd", { paused: newVal });
  };

  const toggleAutoStart = (e) => {
    const newVal = e.target.checked;
    setAutoStart(newVal);
    invoke("set_startup_cmd", { enabled: newVal });
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
    saveConfigWithLatest({ mappings: newMappings });
  };

  const removeMapping = (key) => {
    const newMappings = { ...mappings };
    delete newMappings[key];
    setMappings(newMappings);
    saveConfigWithLatest({ mappings: newMappings });
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
        saveConfigWithLatest({ mappings: newMappings });
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
            <YatsLogo />
            <h2>About YATS</h2>
            <p><strong>YATS</strong> stands for:</p>
            <p className="yats-full-name">Yet Another Touchpad Shortcut</p>
            <p className="version-info">Version 1.3.3</p>
            <button className="btn-close-modal" onClick={() => setShowAbout(false)}>Close</button>
          </div>
        </div>
      )}

      {listeningForKey && (
        <div className="modal-overlay">
          <div className="modal-content">
            <div className="recording-icon">●</div>
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

      <LegacyScrollSettings
        visible={showLegacySettings}
        onClose={() => setShowLegacySettings(false)}
        scrollSensitivity={scrollSensitivity}
        setScrollSensitivity={setScrollSensitivity}
        scrollSpeed={scrollSpeed}
        setScrollSpeed={setScrollSpeed}
        scrollInvert={scrollInvert}
        setScrollInvert={setScrollInvert}
        linuxMinSpeed={linuxMinSpeed}
        setLinuxMinSpeed={setLinuxMinSpeed}
        linuxMinDistance={linuxMinDistance}
        linuxMinScrollSpeed={linuxMinScrollSpeed}
        linuxMaxScrollSpeed={linuxMaxScrollSpeed}
        linuxScrollCurve={linuxScrollCurve}
        saveConfigWithLatest={saveConfigWithLatest}
      />

      {showScrollTuning && (
        <div className="modal-overlay" onClick={() => setShowScrollTuning(false)}>
          <div className="modal-content scroll-tuning-modal" onClick={(e) => e.stopPropagation()} style={{ maxWidth: "600px", width: "95%" }}>
            <h2>Scroll Tuning</h2>
            <div className="modal-body">
              <div className="minor-settings" style={{ marginBottom: "16px" }}>
                <label className="checkbox-item">
                  <input
                    type="checkbox"
                    checked={!scrollInvert}
                    onChange={(e) => {
                      const val = !e.target.checked;
                      setScrollInvert(val);
                      saveConfigWithLatest({ scrollInvert: val });
                    }}
                  />
                  Natural Scroll Direction
                </label>
              </div>
              <div className="tuning-grid">
                <div className="setting-item">
                  <label className="setting-label">Min Mouse Distance (px)</label>
                  <p className="setting-hint">Ignore small movements.</p>
                  <div className="setting-row">
                    <input
                      type="range" min="0" max="100" step="1"
                      value={linuxMinDistance}
                      onChange={(e) => {
                        const val = parseInt(e.target.value);
                        setLinuxMinDistance(val);
                        saveConfigWithLatest({ linuxMinDistance: val });
                      }}
                    />
                    <input type="number" min="0" value={linuxMinDistance} onChange={(e) => {
                      let val = parseInt(e.target.value);
                      if (isNaN(val) || val < 0) val = 0;
                      setLinuxMinDistance(val);
                      saveConfigWithLatest({ linuxMinDistance: val });
                    }} />
                  </div>
                </div>

                <div className="setting-item">
                  <label className="setting-label">Min Scroll Output Speed</label>
                  <p className="setting-hint">Minimum scrolling velocity.</p>
                  <div className="setting-row">
                    <input
                      type="range" min="0" max="100" step="1"
                      value={linuxMinScrollSpeed}
                      onChange={(e) => {
                        const val = parseFloat(e.target.value);
                        setLinuxMinScrollSpeed(val);
                        saveConfigWithLatest({ linuxMinScrollSpeed: val });
                      }}
                    />
                    <input type="number" min="0" value={linuxMinScrollSpeed} onChange={(e) => {
                      let val = parseFloat(e.target.value);
                      if (isNaN(val) || val < 0) val = 0;
                      setLinuxMinScrollSpeed(val);
                      saveConfigWithLatest({ linuxMinScrollSpeed: val });
                    }} />
                  </div>
                </div>

                <div className="setting-item full-width">
                  <label className="setting-label">Max Scroll Output Speed</label>
                  <p className="setting-hint">Caps the scroll acceleration (Curve Y-Axis).</p>
                  <div className="setting-row">
                    <input
                      type="range" min="10" max="2000" step="10"
                      value={linuxMaxScrollSpeed}
                      onChange={(e) => {
                        const val = parseFloat(e.target.value);
                        setLinuxMaxScrollSpeed(val);
                        // Also clamp the existing curve points if they exceed the new max
                        const clamped = linuxScrollCurve.map(p => [p[0], Math.min(p[1], val)]);
                        setLinuxScrollCurve(clamped);
                        saveConfigWithLatest({ linuxMaxScrollSpeed: val, linuxScrollCurve: clamped });
                      }}
                    />
                    <input type="number" min="10" max="2000" value={linuxMaxScrollSpeed} onChange={(e) => {
                      let val = parseFloat(e.target.value);
                      if (isNaN(val) || val < 10) val = 10;
                      if (val > 2000) val = 2000;
                      setLinuxMaxScrollSpeed(val);
                      const clamped = linuxScrollCurve.map(p => [p[0], Math.min(p[1], val)]);
                      setLinuxScrollCurve(clamped);
                      saveConfigWithLatest({ linuxMaxScrollSpeed: val, linuxScrollCurve: clamped });
                    }} />
                  </div>
                </div>
              </div>

              <div className="curve-section">
                <label className="setting-label">Acceleration Curve</label>
                <div className="curve-editor-container">
                  <CurveEditor
                    points={linuxScrollCurve}
                    maxY={linuxMaxScrollSpeed}
                    onChange={(newPoints) => {
                      const sorted = [...newPoints].sort((a, b) => a[0] - b[0]);
                      setLinuxScrollCurve(sorted);
                      saveConfigWithLatest({ linuxScrollCurve: sorted });
                    }}
                  />
                </div>
                <p className="hint-text">Drag points to adjust. Click line to add. Right-click to remove.</p>
              </div>


            </div>
            <div className="modal-footer">
              <button className="btn-close-modal" onClick={() => setShowScrollTuning(false)}>Done</button>
            </div>
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
                    saveConfigWithLatest({ releaseDelay: val });
                  }}
                />
              </label>
            </div>
            {/* Advanced Settings ボタンは非表示（コードは残しておく）
            <div style={{ marginTop: "10px", textAlign: "right" }}>
              <button
                className="btn-global-config"
                style={{ fontSize: "0.8rem", padding: "4px 8px" }}
                onClick={() => setShowLegacySettings(true)}
              >
                Advanced Settings...
              </button>
            </div>
            */}
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
                          <button className="btn-global-config" onClick={() => setShowScrollTuning(true)}>
                            Scroll Tuning...
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
