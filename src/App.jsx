import { useState, useEffect, useMemo, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { currentMonitor } from "@tauri-apps/api/window";
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

const clamp = (value, min, max) => Math.max(min, Math.min(max, value));

const defaultScrollCurve = [
  [0, 2], [1305, 4], [1730, 21], [2000, 79]
];

const getAutomaticMaxInputSpeed = async () => {
  try {
    const monitor = await currentMonitor();
    const width = monitor?.size.width;
    if (Number.isFinite(width) && width > 0) {
      return clamp(Math.round(width * 3 / 100) * 100, 1000, 20000);
    }
  } catch {
    // Use the default when the monitor is unavailable.
  }
  return 2000;
};

const CurveEditor = ({ points, onChange, maxX = 2000, minY = 0, maxY = 2000, livePoint }) => {
  const [draggingIdx, setDraggingIdx] = useState(null);
  const svgRef = useRef(null);
  const width = 480;
  const height = 300;
  const margin = 40;

  const yRange = maxY - minY;

  const toSvgX = (x) => margin + (x / maxX) * (width - 2 * margin);
  const toSvgY = (y) => height - margin - ((y - minY) / (yRange || 1)) * (height - 2 * margin);
  const fromSvgX = (sx) => ((sx - margin) / (width - 2 * margin)) * maxX;
  const fromSvgY = (sy) => minY + ((height - margin - sy) / (height - 2 * margin)) * yRange;

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
    const y = Math.round(clamp(fromSvgY(sy), minY, maxY));

    const newPoints = [...sortedPoints];

    // Constraint: don't cross neighbors
    if (draggingIdx > 0) x = Math.max(newPoints[draggingIdx - 1][0] + 5, x);
    if (draggingIdx < n - 1) x = Math.min(newPoints[draggingIdx + 1][0] - 5, x);

    newPoints[draggingIdx] = [Math.round(clamp(x, 0, maxX)), y];
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

    const x = Math.round(clamp(fromSvgX(sx), 0, maxX));
    const y = Math.round(clamp(fromSvgY(sy), minY, maxY));

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
  const liveX = livePoint && toSvgX(clamp(livePoint.x, 0, maxX));
  const liveY = livePoint && toSvgY(clamp(livePoint.y, minY, maxY));
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
        {[0, 0.25, 0.5, 0.75, 1].map(v => {
          const y = minY + v * yRange;
          const svgY = toSvgY(y);
          return (
            <g key={v}>
              <line x1={margin} y1={svgY} x2={width - margin} y2={svgY} stroke="rgba(255,255,255,0.05)" strokeWidth="1" />
              <line x1={toSvgX(v * maxX)} y1={margin} x2={toSvgX(v * maxX)} y2={height - margin} stroke="rgba(255,255,255,0.05)" strokeWidth="1" />
              <text x={margin - 8} y={svgY} fill="#666" fontSize="10" textAnchor="end" alignmentBaseline="middle">{Math.round(y)}</text>
              <text x={toSvgX(v * maxX)} y={height - margin + 18} fill="#666" fontSize="10" textAnchor="middle">{Math.round(v * maxX)}</text>
            </g>
          );
        })}

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
        {livePoint && (
          <g pointerEvents="none" aria-hidden="true">
            <line x1={liveX} y1={liveY} x2={liveX} y2={height - margin} stroke="#ffcf5a" strokeOpacity={0.6} strokeWidth={1} />
            <circle cx={liveX} cy={liveY} r={12} fill="#ffcf5a" fillOpacity={0.25} />
            <circle cx={liveX} cy={liveY} r={6} fill="#ffcf5a" stroke="#fff" strokeWidth={2} />
          </g>
        )}
      </svg>
    </div>
  );
};

function App() {
  const [mappings, setMappings] = useState({});
  const [isTouched, setIsTouched] = useState(false);
  const [releaseDelay, setReleaseDelay] = useState(100);
  const [showAbout, setShowAbout] = useState(false);
  const [appVersion, setAppVersion] = useState("1.3.7");
  const [listeningForKey, setListeningForKey] = useState(false);
  const [sortField, setSortField] = useState("key");
  const [sortOrder, setSortOrder] = useState("asc");
  const [isPaused, setIsPaused] = useState(false);
  const [autoStart, setAutoStart] = useState(false);
  const [palmRejection, setPalmRejection] = useState(true);
  const [palmRejectionDelay, setPalmRejectionDelay] = useState(500);
  const [recordingMacroKey, setRecordingMacroKey] = useState(null);
  const [showScrollTuning, setShowScrollTuning] = useState(false);
  const [livePoint, setLivePoint] = useState(null);
  const livePointTimer = useRef(null);
  const [scrollInvert, setScrollInvert] = useState(true);
  const [scrollSensitivity, setScrollSensitivity] = useState(1);
  const [scrollSpeed, setScrollSpeed] = useState(25);
  const [linuxMinScrollSpeed, setLinuxMinScrollSpeed] = useState(1);
  const [linuxMaxScrollSpeed, setLinuxMaxScrollSpeed] = useState(100);
  const [linuxMaxInputSpeed, setLinuxMaxInputSpeed] = useState(2000);
  const [linuxScrollCurve, setLinuxScrollCurve] = useState(defaultScrollCurve);
  const [showLegacySettings, setShowLegacySettings] = useState(false);

  useEffect(() => {
    if (!showScrollTuning) return;

    let closed = false;
    let unlisten;
    listen("scroll-activity", ({ payload }) => {
      if (closed) return;
      setLivePoint({ x: payload.input, y: payload.output });
      clearTimeout(livePointTimer.current);
      livePointTimer.current = setTimeout(() => {
        livePointTimer.current = null;
        setLivePoint(null);
      }, 500);
    }).then((dispose) => {
      if (closed) {
        dispose();
        return;
      }
      unlisten = dispose;
      return invoke("set_curve_monitoring", { enabled: true });
    }).catch(console.error);

    return () => {
      closed = true;
      unlisten?.();
      invoke("set_curve_monitoring", { enabled: false }).catch(console.error);
      clearTimeout(livePointTimer.current);
      livePointTimer.current = null;
      setLivePoint(null);
    };
  }, [showScrollTuning]);

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
        linux_min_scroll_speed: overrides.linuxMinScrollSpeed !== undefined ? overrides.linuxMinScrollSpeed : linuxMinScrollSpeed,
        linux_max_scroll_speed: overrides.linuxMaxScrollSpeed !== undefined ? overrides.linuxMaxScrollSpeed : linuxMaxScrollSpeed,
        linux_max_input_speed: overrides.linuxMaxInputSpeed ?? linuxMaxInputSpeed,
        linux_use_scroll_curve: true, // Fixed as baseline
        linux_scroll_curve: overrides.linuxScrollCurve !== undefined ? overrides.linuxScrollCurve : linuxScrollCurve,
        palm_rejection: overrides.palmRejection !== undefined ? overrides.palmRejection : palmRejection,
        palm_rejection_delay_ms: overrides.palmRejectionDelay !== undefined ? parseInt(overrides.palmRejectionDelay) : palmRejectionDelay,
      };

      await invoke("set_config", { newConfig: updated });
    } catch (err) {
      console.error("Failed to save config:", err);
    }
  };

  const updateScrollOutputRange = (min, max) => {
    const oldRange = linuxMaxScrollSpeed - linuxMinScrollSpeed;
    const curve = linuxScrollCurve.map(([x, y]) => {
      const scaledY = oldRange > 0
        ? min + (y - linuxMinScrollSpeed) * (max - min) / oldRange
        : y;
      return [x, clamp(Math.round(scaledY), min, max)];
    });

    setLinuxMinScrollSpeed(min);
    setLinuxMaxScrollSpeed(max);
    setLinuxScrollCurve(curve);
    saveConfigWithLatest({
      linuxMinScrollSpeed: min,
      linuxMaxScrollSpeed: max,
      linuxScrollCurve: curve,
    });
  };

  const handleMinScrollSpeedChange = (val) => {
    const rounded = Math.round(val);
    if (!Number.isFinite(rounded)) return;
    const min = clamp(rounded, 0, linuxMaxScrollSpeed - 1);
    updateScrollOutputRange(min, linuxMaxScrollSpeed);
  };

  const handleMaxScrollSpeedChange = (val) => {
    const rounded = Math.round(val);
    if (!Number.isFinite(rounded)) return;
    const max = clamp(rounded, linuxMinScrollSpeed + 1, 2000);
    updateScrollOutputRange(linuxMinScrollSpeed, max);
  };

  const handleMaxInputSpeedChange = (val) => {
    const rounded = Math.round(val);
    if (!Number.isFinite(rounded)) return;
    const newMax = clamp(rounded, 1000, 20000);
    if (newMax === linuxMaxInputSpeed) return;

    const minSpacing = 10;
    if ((linuxScrollCurve.length - 1) * minSpacing > newMax) return;

    const curve = linuxScrollCurve
      .map(([x, y]) => [clamp(Math.round(x * newMax / linuxMaxInputSpeed), 0, newMax), y])
      .sort((a, b) => a[0] - b[0]);

    for (let i = 0; i < curve.length; i++) {
      const lower = i === 0 ? 0 : curve[i - 1][0] + minSpacing;
      // Leave room for the remaining points near the right edge.
      const upper = newMax - (curve.length - 1 - i) * minSpacing;
      curve[i][0] = clamp(curve[i][0], lower, upper);
    }

    setLinuxMaxInputSpeed(newMax);
    setLinuxScrollCurve(curve);
    saveConfigWithLatest({ linuxMaxInputSpeed: newMax, linuxScrollCurve: curve });
  };

  const loadConfig = async () => {
    try {
      const config = await invoke("get_config");
      const maxInput = config.linux_max_input_speed > 0
        ? config.linux_max_input_speed
        : await getAutomaticMaxInputSpeed();
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
      setScrollSpeed(config.scroll_speed ?? 25);
      setScrollInvert(config.scroll_invert ?? true);
      const min = clamp(Math.round(config.linux_min_scroll_speed ?? 1), 0, 1999);
      const max = clamp(Math.round(config.linux_max_scroll_speed ?? 100), min + 1, 2000);
      const useDefaultCurve = config.linux_scroll_curve_is_default || !config.linux_scroll_curve?.length;
      const curve = useDefaultCurve ? defaultScrollCurve : config.linux_scroll_curve;
      setLinuxMinScrollSpeed(min);
      setLinuxMaxScrollSpeed(max);
      setLinuxMaxInputSpeed(maxInput);
      setLinuxScrollCurve(curve.map(([x, y]) => [
        Math.round(useDefaultCurve ? x * maxInput / 2000 : clamp(x, 0, maxInput)),
        clamp(Math.round(y), min, max),
      ]));
      setPalmRejection(config.palm_rejection ?? true);
      setPalmRejectionDelay(config.palm_rejection_delay_ms ?? 500);
    } catch (err) {
      console.error("Failed to load config:", err);
    }
  };

  useEffect(() => {
    loadConfig();
    invoke("get_paused_cmd").then(setIsPaused);
    listen("pause-status", (event) => setIsPaused(event.payload));
    invoke("get_startup_status_cmd").then(setAutoStart);
    invoke("get_app_version").then(setAppVersion).catch(console.error);

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
    } else if (actionType === "WindowDrag") {
      finalValue = undefined;
    } else if (actionType === "MouseScroll") {
      if (typeof actionValue !== "object" || !actionValue) finalValue = { sensitivity: 100 };
    }

    const newAction = finalValue === undefined
      ? { type: actionType }
      : { type: actionType, value: finalValue };
    const newMappings = { ...mappings, [key]: newAction };
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
            <p className="version-info">Version {appVersion}</p>
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
        linuxMinScrollSpeed={linuxMinScrollSpeed}
        linuxMaxScrollSpeed={linuxMaxScrollSpeed}
        linuxMaxInputSpeed={linuxMaxInputSpeed}
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
                <div className="setting-item full-width">
                  <label className="setting-label">Min Scroll Output Speed</label>
                  <p className="setting-hint">Lower bound of the curve's output axis.</p>
                  <div className="setting-row">
                    <input
                      type="range" min="0" max={linuxMaxScrollSpeed - 1} step="1"
                      value={linuxMinScrollSpeed}
                      onChange={(e) => handleMinScrollSpeedChange(e.target.valueAsNumber)}
                    />
                    <input
                      type="number" min="0" max={linuxMaxScrollSpeed - 1} step="1"
                      value={linuxMinScrollSpeed}
                      onChange={(e) => handleMinScrollSpeedChange(e.target.valueAsNumber)}
                    />
                  </div>
                </div>

                <div className="setting-item full-width">
                  <label className="setting-label">Max Scroll Output Speed</label>
                  <p className="setting-hint">Upper bound of the curve's output axis. Changing it rescales the curve.</p>
                  <div className="setting-row">
                    <input
                      type="range" min={linuxMinScrollSpeed + 1} max="2000" step="1"
                      value={linuxMaxScrollSpeed}
                      onChange={(e) => handleMaxScrollSpeedChange(e.target.valueAsNumber)}
                    />
                    <input
                      type="number" min={linuxMinScrollSpeed + 1} max="2000" step="1"
                      value={linuxMaxScrollSpeed}
                      onChange={(e) => handleMaxScrollSpeedChange(e.target.valueAsNumber)}
                    />
                  </div>
                </div>

                <div className="setting-item full-width">
                  <label className="setting-label">Max Input Speed</label>
                  <p className="setting-hint">Upper bound of the curve's input axis. Changing it rescales the curve.</p>
                  <div className="setting-row">
                    <input
                      type="range" min="1000" max="20000" step="100"
                      value={linuxMaxInputSpeed}
                      onChange={(e) => handleMaxInputSpeedChange(e.target.valueAsNumber)}
                    />
                    <input
                      type="number" min="1000" max="20000" step="1"
                      value={linuxMaxInputSpeed}
                      onChange={(e) => handleMaxInputSpeedChange(e.target.valueAsNumber)}
                    />
                  </div>
                </div>
              </div>

              <div className="curve-section">
                <label className="setting-label">Acceleration Curve</label>
                <div className="curve-editor-container">
                  <CurveEditor
                    points={linuxScrollCurve}
                    livePoint={livePoint}
                    maxX={linuxMaxInputSpeed}
                    minY={linuxMinScrollSpeed}
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
        <div className="header-top-row">
          <div className="title-group">
            <YatsLogo />
            <h1>YATS Settings <span className="about-link" onClick={() => setShowAbout(true)}>?</span></h1>
          </div>
          <div className="status-container">
            <div className={`status-badge ${isTouched ? "active" : ""}`}>
              {isTouched ? "TOUCHED" : "IDLE"}
            </div>
          </div>
        </div>
        <div className="header-controls">
          <label className="header-control-label">General Settings</label>
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
            <label className="header-control-item">
              <input
                type="checkbox"
                checked={palmRejection}
                onChange={(e) => {
                  const val = e.target.checked;
                  setPalmRejection(val);
                  saveConfigWithLatest({ palmRejection: val });
                }}
              />
              Palm Rejection
            </label>
            {palmRejection && (
              <label className="header-control-item delay-control">
                Palm Delay: <span className="value-display">{palmRejectionDelay}</span>ms
                <input
                  type="range" min="100" max="2000" step="50"
                  value={palmRejectionDelay}
                  onChange={(e) => {
                    const val = parseInt(e.target.value);
                    setPalmRejectionDelay(val);
                    saveConfigWithLatest({ palmRejectionDelay: val });
                  }}
                />
              </label>
            )}
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
                        <option value="WindowDrag">Window Drag</option>
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
