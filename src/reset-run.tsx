import { useCallback, useEffect, useRef, useState } from "react";

import type { MonitorPreferences } from "./monitor-controls";
import type { CodexLimitReached } from "./limit-game";
import { formatResetCountdown } from "./limit-game";

type GamePhase = "ready" | "running" | "paused" | "crashed" | "checkpoint";
type Obstacle = { x: number; width: number; height: number; glyph: string };
type GameRuntime = {
  buddyY: number;
  velocityY: number;
  obstacles: Obstacle[];
  spawnIn: number;
  distance: number;
  score: number;
  speed: number;
};

const GAME_WIDTH = 328;
const GAME_HEIGHT = 196;
const GROUND_Y = 158;
const BUDDY_X = 42;
const BUDDY_SIZE = 30;
const TARGET_DISTANCE = 16_000;
const BEST_SCORE_KEY = "quotabuddy.reset-run.best-score.v1";

const gameCopy = {
  en: {
    back: "Back to usage",
    eyebrow: "CODEX COOLDOWN",
    title: "Run to RESET",
    realUnlock: "REAL UNLOCK",
    distance: "DISTANCE TO RESET",
    score: "SCORE",
    best: "BEST",
    ready: "Buddy is ready",
    readyDescription: "Jump the token blocks and reach the RESET portal.",
    start: "Start run",
    paused: "Run paused",
    pausedDescription: "Resume when you are ready.",
    resume: "Resume",
    crashed: "Context collision",
    crashedDescription: "Buddy hit a token block. The portal is still ahead.",
    retry: "Try again",
    checkpoint: "RESET checkpoint reached",
    checkpointDescription: "Great run. Your real quota still unlocks at the time above.",
    runAgain: "Run again",
    controls: "SPACE / ↑ / CLICK TO JUMP",
    truth: "Game progress never changes your real Codex quota.",
    session: "5h session",
    weekly: "weekly",
    canvas: "Reset Run game. Press Space, Arrow Up, or click to jump.",
  },
  ptBr: {
    back: "Voltar ao uso",
    eyebrow: "CODEX EM COOLDOWN",
    title: "Corra até o RESET",
    realUnlock: "DESBLOQUEIO REAL",
    distance: "DISTÂNCIA ATÉ O RESET",
    score: "PONTOS",
    best: "RECORDE",
    ready: "Buddy está pronto",
    readyDescription: "Pule os blocos de tokens e alcance o portal RESET.",
    start: "Começar corrida",
    paused: "Corrida pausada",
    pausedDescription: "Continue quando estiver pronto.",
    resume: "Continuar",
    crashed: "Colisão de contexto",
    crashedDescription: "Buddy bateu num bloco de tokens. O portal continua à frente.",
    retry: "Tentar novamente",
    checkpoint: "Checkpoint RESET alcançado",
    checkpointDescription: "Boa corrida. Sua quota real ainda volta no horário acima.",
    runAgain: "Correr novamente",
    controls: "ESPAÇO / ↑ / CLIQUE PARA PULAR",
    truth: "O progresso do jogo nunca altera sua quota real do Codex.",
    session: "sessão de 5h",
    weekly: "semanal",
    canvas: "Jogo Reset Run. Pressione Espaço, seta para cima ou clique para pular.",
  },
} as const;

function createRuntime(jumpImmediately = false): GameRuntime {
  return {
    buddyY: GROUND_Y - BUDDY_SIZE,
    velocityY: jumpImmediately ? -390 : 0,
    obstacles: [],
    spawnIn: 1.35,
    distance: 0,
    score: 0,
    speed: 185,
  };
}

function readBestScore(): number {
  try {
    const value = Number(window.localStorage.getItem(BEST_SCORE_KEY));
    return Number.isFinite(value) && value > 0 ? Math.floor(value) : 0;
  } catch {
    return 0;
  }
}

function saveBestScore(value: number) {
  try {
    window.localStorage.setItem(BEST_SCORE_KEY, String(value));
  } catch {
    // Local record is optional. The game remains fully playable without storage.
  }
}

export function ResetRun({ limit, language, onBack, onResetDue }: {
  limit: CodexLimitReached;
  language: MonitorPreferences["language"];
  onBack: () => void;
  onResetDue: () => void;
}) {
  const text = language === "ptBr" ? gameCopy.ptBr : gameCopy.en;
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const runtimeRef = useRef<GameRuntime>(createRuntime());
  const phaseRef = useRef<GamePhase>("ready");
  const resetRefreshRequested = useRef(false);
  const [phase, setPhase] = useState<GamePhase>("ready");
  const [score, setScore] = useState(0);
  const [bestScore, setBestScore] = useState(readBestScore);
  const [progress, setProgress] = useState(0);
  const [now, setNow] = useState(Date.now());

  useEffect(() => {
    phaseRef.current = phase;
  }, [phase]);

  useEffect(() => {
    const interval = window.setInterval(() => setNow(Date.now()), 1_000);
    return () => window.clearInterval(interval);
  }, []);

  useEffect(() => {
    const resetTime = new Date(limit.effectiveResetAt).getTime();
    if (now < resetTime || resetRefreshRequested.current) return;
    resetRefreshRequested.current = true;
    onResetDue();
  }, [limit.effectiveResetAt, now, onResetDue]);

  const finishRound = useCallback((nextPhase: "crashed" | "checkpoint") => {
    const nextScore = Math.floor(runtimeRef.current.score);
    setScore(nextScore);
    setProgress(Math.min(100, Math.round((runtimeRef.current.distance / TARGET_DISTANCE) * 100)));
    setBestScore((current) => {
      const next = Math.max(current, nextScore);
      if (next !== current) saveBestScore(next);
      return next;
    });
    setPhase(nextPhase);
  }, []);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    drawScene(canvas, runtimeRef.current);
    if (phase !== "running") return;

    let animationFrame = 0;
    let previous = performance.now();
    let lastUiUpdate = 0;

    const frame = (timestamp: number) => {
      const elapsed = Math.min(0.034, Math.max(0, (timestamp - previous) / 1_000));
      previous = timestamp;
      const runtime = runtimeRef.current;

      runtime.velocityY += 940 * elapsed;
      runtime.buddyY = Math.min(GROUND_Y - BUDDY_SIZE, runtime.buddyY + runtime.velocityY * elapsed);
      if (runtime.buddyY >= GROUND_Y - BUDDY_SIZE) runtime.velocityY = 0;

      runtime.distance += runtime.speed * elapsed;
      runtime.score = Math.floor(runtime.distance / 9);
      runtime.speed = Math.min(270, 185 + runtime.distance / 85);
      runtime.spawnIn -= elapsed;

      if (runtime.spawnIn <= 0 && runtime.distance < TARGET_DISTANCE - 620) {
        const tall = Math.random() > 0.68;
        runtime.obstacles.push({
          x: GAME_WIDTH + 12,
          width: tall ? 22 : 18,
          height: tall ? 37 : 26,
          glyph: ["{}", "<>", "100%", "CTX"][Math.floor(Math.random() * 4)],
        });
        const difficulty = Math.min(0.38, runtime.distance / 20_000);
        runtime.spawnIn = 1.18 + Math.random() * 0.72 - difficulty;
      }

      runtime.obstacles.forEach((obstacle) => { obstacle.x -= runtime.speed * elapsed; });
      runtime.obstacles = runtime.obstacles.filter((obstacle) => obstacle.x + obstacle.width > -8);

      const buddyBox = { x: BUDDY_X + 4, y: runtime.buddyY + 3, width: BUDDY_SIZE - 8, height: BUDDY_SIZE - 4 };
      const collided = runtime.obstacles.some((obstacle) => rectanglesOverlap(buddyBox, {
        x: obstacle.x + 2,
        y: GROUND_Y - obstacle.height + 2,
        width: obstacle.width - 4,
        height: obstacle.height - 2,
      }));

      drawScene(canvas, runtime);

      if (collided) {
        finishRound("crashed");
        return;
      }
      if (runtime.distance >= TARGET_DISTANCE) {
        runtime.distance = TARGET_DISTANCE;
        drawScene(canvas, runtime);
        finishRound("checkpoint");
        return;
      }

      if (timestamp - lastUiUpdate > 90) {
        lastUiUpdate = timestamp;
        setScore(runtime.score);
        setProgress(Math.min(100, Math.round((runtime.distance / TARGET_DISTANCE) * 100)));
      }
      animationFrame = window.requestAnimationFrame(frame);
    };

    animationFrame = window.requestAnimationFrame(frame);
    return () => window.cancelAnimationFrame(animationFrame);
  }, [finishRound, phase]);

  const startGame = useCallback((jumpImmediately = false) => {
    runtimeRef.current = createRuntime(jumpImmediately);
    setScore(0);
    setProgress(0);
    setPhase("running");
  }, []);

  const jump = useCallback(() => {
    const runtime = runtimeRef.current;
    if (runtime.buddyY >= GROUND_Y - BUDDY_SIZE - 1) runtime.velocityY = -390;
  }, []);

  const handleAction = useCallback(() => {
    if (phase === "running") jump();
    else if (phase === "paused") setPhase("running");
    else startGame(true);
  }, [jump, phase, startGame]);

  useEffect(() => {
    const handleKey = (event: KeyboardEvent) => {
      if (!["Space", "ArrowUp", "KeyW"].includes(event.code)) return;
      const target = event.target instanceof Element ? event.target : null;
      if (target?.closest("button, input, select, textarea, a[href]") && target !== canvasRef.current) return;
      event.preventDefault();
      handleAction();
    };
    window.addEventListener("keydown", handleKey);
    return () => window.removeEventListener("keydown", handleKey);
  }, [handleAction]);

  useEffect(() => {
    const pause = () => {
      if (phaseRef.current === "running") setPhase("paused");
    };
    const handleVisibility = () => { if (document.hidden) pause(); };
    window.addEventListener("blur", pause);
    document.addEventListener("visibilitychange", handleVisibility);
    return () => {
      window.removeEventListener("blur", pause);
      document.removeEventListener("visibilitychange", handleVisibility);
    };
  }, []);

  const overlay = phase === "ready"
    ? { title: text.ready, description: text.readyDescription, action: text.start }
    : phase === "paused"
      ? { title: text.paused, description: text.pausedDescription, action: text.resume }
      : phase === "crashed"
        ? { title: text.crashed, description: text.crashedDescription, action: text.retry }
        : phase === "checkpoint"
          ? { title: text.checkpoint, description: text.checkpointDescription, action: text.runAgain }
          : null;

  return <section className="reset-run" aria-labelledby="reset-run-title">
    <header className="reset-run-header">
      <button className="game-back" type="button" aria-label={text.back} title={text.back} onClick={onBack}><BackArrow /></button>
      <div className="reset-run-title"><span>{text.eyebrow}</span><h1 id="reset-run-title">{text.title}</h1></div>
      <div className="real-reset-clock"><span>{text.realUnlock}</span><strong>{formatResetCountdown(limit.effectiveResetAt, now)}</strong></div>
    </header>

    <div className="reset-game-card">
      <div className="game-hud">
        <div className="game-progress-copy"><span>{text.distance}</span><strong>{progress}%</strong></div>
        <div className="game-score"><span>{text.score} <strong>{score}</strong></span><span>{text.best} <strong>{bestScore}</strong></span></div>
      </div>
      <div className="reset-progress-track" aria-hidden="true"><span style={{ width: `${progress}%` }} /></div>

      <div className="game-stage" onPointerDown={(event) => {
        if ((event.target as HTMLElement).closest("button")) return;
        event.preventDefault();
        handleAction();
      }}>
        <canvas ref={canvasRef} width={GAME_WIDTH * 2} height={GAME_HEIGHT * 2} role="button" tabIndex={0} aria-label={text.canvas} />
        {overlay ? <div className={`game-overlay ${phase}`} role="status">
          <BuddyBadge />
          <strong>{overlay.title}</strong>
          <p>{overlay.description}</p>
          <button type="button" onClick={() => phase === "paused" ? setPhase("running") : startGame(false)}>{overlay.action}</button>
        </div> : null}
      </div>

      <div className="game-meta">
        <span className="game-controls">{text.controls}</span>
        <div className="reached-limit-tags">{limit.reached.map((item) => <span key={item.slot}>{item.slot === "session" ? text.session : text.weekly}</span>)}</div>
      </div>
      <p className="game-truth">{text.truth}</p>
    </div>
  </section>;
}

function rectanglesOverlap(a: { x: number; y: number; width: number; height: number }, b: { x: number; y: number; width: number; height: number }) {
  return a.x < b.x + b.width && a.x + a.width > b.x && a.y < b.y + b.height && a.y + a.height > b.y;
}

function drawScene(canvas: HTMLCanvasElement, runtime: GameRuntime) {
  const context = canvas.getContext("2d");
  if (!context) return;
  context.setTransform(2, 0, 0, 2, 0, 0);
  context.clearRect(0, 0, GAME_WIDTH, GAME_HEIGHT);

  const background = context.createLinearGradient(0, 0, 0, GAME_HEIGHT);
  background.addColorStop(0, "#07111d");
  background.addColorStop(0.62, "#0b1e2b");
  background.addColorStop(1, "#102532");
  context.fillStyle = background;
  context.fillRect(0, 0, GAME_WIDTH, GAME_HEIGHT);

  context.save();
  context.globalAlpha = 0.35;
  for (let index = 0; index < 20; index += 1) {
    const x = ((index * 67 - runtime.distance * (0.07 + (index % 3) * 0.025)) % (GAME_WIDTH + 28) + GAME_WIDTH + 28) % (GAME_WIDTH + 28) - 14;
    const y = 18 + ((index * 41) % 104);
    context.fillStyle = index % 4 === 0 ? "#ffb329" : "#35d5f4";
    context.fillRect(x, y, index % 4 === 0 ? 2 : 1, index % 4 === 0 ? 2 : 1);
  }
  context.restore();

  context.save();
  context.strokeStyle = "rgba(53, 213, 244, 0.11)";
  context.lineWidth = 1;
  const gridOffset = (runtime.distance * 0.18) % 28;
  for (let x = -gridOffset; x <= GAME_WIDTH; x += 28) {
    context.beginPath(); context.moveTo(x, 112); context.lineTo(x, GROUND_Y + 22); context.stroke();
  }
  for (let y = 116; y <= GROUND_Y + 22; y += 14) {
    context.beginPath(); context.moveTo(0, y); context.lineTo(GAME_WIDTH, y); context.stroke();
  }
  context.restore();

  const portalReveal = Math.max(0, Math.min(1, (runtime.distance - (TARGET_DISTANCE - 980)) / 980));
  if (portalReveal > 0) drawPortal(context, GAME_WIDTH + 22 - portalReveal * (GAME_WIDTH - 70));

  runtime.obstacles.forEach((obstacle) => drawObstacle(context, obstacle));
  drawBuddy(context, BUDDY_X, runtime.buddyY, runtime.velocityY !== 0);

  context.strokeStyle = "rgba(138, 234, 255, 0.62)";
  context.lineWidth = 1.5;
  context.beginPath(); context.moveTo(0, GROUND_Y + 0.5); context.lineTo(GAME_WIDTH, GROUND_Y + 0.5); context.stroke();
  context.fillStyle = "rgba(53, 213, 244, 0.12)";
  context.fillRect(0, GROUND_Y + 2, GAME_WIDTH, 24);

  context.save();
  context.globalAlpha = 0.09;
  context.fillStyle = "#ffffff";
  for (let y = 0; y < GAME_HEIGHT; y += 4) context.fillRect(0, y, GAME_WIDTH, 1);
  context.restore();
}

function drawBuddy(context: CanvasRenderingContext2D, x: number, y: number, airborne: boolean) {
  context.save();
  context.shadowColor = "rgba(53, 213, 244, 0.65)";
  context.shadowBlur = 10;
  context.fillStyle = "#112d3a";
  context.strokeStyle = "#35d5f4";
  context.lineWidth = 2;
  context.beginPath(); context.roundRect(x, y + 4, BUDDY_SIZE, BUDDY_SIZE - 7, 7); context.fill(); context.stroke();
  context.shadowBlur = 0;
  context.fillStyle = "#8aeaff";
  context.fillRect(x + 7, y + 12, 4, 4);
  context.fillRect(x + 19, y + 12, 4, 4);
  context.strokeStyle = "#ffb329";
  context.beginPath(); context.moveTo(x + 12, y + 21); context.lineTo(x + 18, y + 21); context.stroke();
  context.strokeStyle = "#35d5f4";
  context.beginPath(); context.moveTo(x + 15, y + 4); context.lineTo(x + 15, y); context.lineTo(x + 19, y); context.stroke();
  context.fillStyle = "#35d5f4";
  const footOffset = airborne ? 2 : 0;
  context.fillRect(x + 5, y + BUDDY_SIZE - footOffset, 8, 3);
  context.fillRect(x + 18, y + BUDDY_SIZE - 3 + footOffset, 8, 3);
  context.restore();
}

function drawObstacle(context: CanvasRenderingContext2D, obstacle: Obstacle) {
  const y = GROUND_Y - obstacle.height;
  context.save();
  context.fillStyle = "rgba(255, 113, 105, 0.13)";
  context.strokeStyle = "#ff7169";
  context.lineWidth = 1.5;
  context.beginPath(); context.roundRect(obstacle.x, y, obstacle.width, obstacle.height, 4); context.fill(); context.stroke();
  context.fillStyle = "#ff938d";
  context.font = obstacle.glyph.length > 2 ? "600 6px Consolas" : "600 8px Consolas";
  context.textAlign = "center";
  context.fillText(obstacle.glyph, obstacle.x + obstacle.width / 2, y + obstacle.height / 2 + 3);
  context.restore();
}

function drawPortal(context: CanvasRenderingContext2D, x: number) {
  context.save();
  context.shadowColor = "rgba(255, 179, 41, 0.85)";
  context.shadowBlur = 18;
  context.strokeStyle = "#ffb329";
  context.lineWidth = 4;
  context.beginPath(); context.arc(x, GROUND_Y - 29, 22, Math.PI, 0); context.lineTo(x + 22, GROUND_Y); context.moveTo(x - 22, GROUND_Y); context.lineTo(x - 22, GROUND_Y - 29); context.stroke();
  context.shadowBlur = 0;
  context.fillStyle = "#ffd173";
  context.font = "700 7px Consolas";
  context.textAlign = "center";
  context.fillText("RESET", x, GROUND_Y - 30);
  context.restore();
}

function BuddyBadge() {
  return <span className="buddy-badge" aria-hidden="true"><span className="buddy-antenna" /><span className="buddy-eye left" /><span className="buddy-eye right" /><span className="buddy-mouth" /></span>;
}

function BackArrow() {
  return <svg viewBox="0 0 24 24" aria-hidden="true"><path d="m15 18-6-6 6-6" /></svg>;
}
