import React from 'react';
import {
  AbsoluteFill, Img, Sequence, interpolate, spring, staticFile, useCurrentFrame, useVideoConfig, Easing,
} from 'remotion';

export const C = { bg: '#0c0c0e', panel: '#16161b', line: '#26262e', text: '#f2f2f4', muted: '#9a9ca3', accent: '#f26b21', ok: '#4ade80' };
export const FONT = '"Segoe UI Variable Display", "Segoe UI", system-ui, sans-serif';

export const fadeUp = (frame: number, start: number, fps: number) => {
  const s = spring({ frame: frame - start, fps, config: { damping: 200 } });
  return { opacity: s, transform: `translateY(${(1 - s) * 24}px)` };
};
export const fadeOut = (frame: number, end: number, len = 10) =>
  interpolate(frame, [end - len, end], [1, 0], { extrapolateLeft: 'clamp', extrapolateRight: 'clamp' });

/** 1. What people are tired of, each line crossed out. */
const Problems: React.FC<{ dur: number }> = ({ dur }) => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();
  const items = ['Background services', 'Pop-ups and ads', '"Restart to finish setup"', '~200 MB running all day'];
  return (
    <AbsoluteFill style={{ justifyContent: 'center', alignItems: 'center', opacity: fadeOut(frame, dur) }}>
      <div style={{ width: 1000 }}>
        <div style={{ ...fadeUp(frame, 0, fps), color: C.muted, fontSize: 40, marginBottom: 30 }}>Your mouse needs a driver. Not this:</div>
        {items.map((t, i) => {
          const start = 12 + i * 12;
          const strike = interpolate(frame, [start + 30, start + 42], [0, 100], { extrapolateLeft: 'clamp', extrapolateRight: 'clamp', easing: Easing.out(Easing.cubic) });
          return (
            <div key={t} style={{ ...fadeUp(frame, start, fps), position: 'relative', display: 'inline-block', fontSize: 76, fontWeight: 700, margin: '6px 0', color: strike > 50 ? C.muted : C.text, letterSpacing: -1.5 }}>
              {t}
              <div style={{ position: 'absolute', left: -8, top: '54%', height: 7, borderRadius: 4, background: C.accent, width: `calc(${strike}% + 16px)` }} />
              <br />
            </div>
          );
        })}
      </div>
    </AbsoluteFill>
  );
};

/** 2. Logo and name. */
const Intro: React.FC<{ dur: number }> = ({ dur }) => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();
  const pop = spring({ frame, fps, config: { damping: 14, mass: 0.8 } });
  return (
    <AbsoluteFill style={{ justifyContent: 'center', alignItems: 'center', gap: 56, flexDirection: 'row', opacity: fadeOut(frame, dur) }}>
      <Img src={staticFile('logo.png')} style={{ width: 230, height: 230, borderRadius: 52, transform: `scale(${pop})`, boxShadow: '0 30px 80px #0009' }} />
      <div>
        <div style={{ ...fadeUp(frame, 8, fps), fontSize: 132, fontWeight: 700, letterSpacing: -4, lineHeight: 1 }}>Secretarybird</div>
        <div style={{ ...fadeUp(frame, 18, fps), fontSize: 50, color: C.muted, marginTop: 18 }}>
          Your Razer gear, <span style={{ color: C.accent, fontWeight: 600 }}>minus the bloat.</span>
        </div>
      </div>
    </AbsoluteFill>
  );
};

/** 3. One fact per beat. */
const Fact: React.FC<{ big: string; line: string; dur: number; extra?: React.ReactNode }> = ({ big, line, dur, extra }) => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();
  return (
    <AbsoluteFill style={{ justifyContent: 'center', alignItems: 'center', opacity: fadeOut(frame, dur, 8) }}>
      <div style={{ textAlign: 'center' }}>
        <div style={{ ...fadeUp(frame, 0, fps), fontSize: 150, fontWeight: 750, letterSpacing: -5, lineHeight: 1.05 }}>{big}</div>
        <div style={{ ...fadeUp(frame, 6, fps), fontSize: 46, color: C.muted, marginTop: 22 }}>{line}</div>
        {extra && <div style={{ ...fadeUp(frame, 14, fps), marginTop: 44 }}>{extra}</div>}
      </div>
    </AbsoluteFill>
  );
};

const Terminal: React.FC = () => {
  const frame = useCurrentFrame();
  const lines = [
    ['$ ', 'secbird debloat apply', C.text],
    ['', 'Razer background services:', C.muted],
    ['  ', 'Razer Elevation Service        running  ->  stopped', C.text],
    ['  ', 'Razer Game Manager Service 3   running  ->  stopped', C.text],
    ['', 'Every command is shown first.  `secbird debloat undo` puts it back.', C.ok],
  ] as const;
  return (
    <div style={{ width: 1180, margin: '0 auto', textAlign: 'left', background: C.panel, border: `1px solid ${C.line}`, borderRadius: 16, padding: '26px 32px', font: '30px/1.6 Consolas, monospace' }}>
      {lines.map(([p, t, color], i) => {
        const shown = Math.max(0, Math.min(t.length, Math.floor((frame - 16 - i * 9) * (i === 0 ? 1.4 : 6))));
        return <div key={i} style={{ color, whiteSpace: 'pre', minHeight: 48 }}>{shown > 0 ? p + t.slice(0, shown) : ''}</div>;
      })}
    </div>
  );
};

/** 4. The editor itself. */
const Screens: React.FC<{ dur: number }> = ({ dur }) => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();
  const shots = ['overview.png', 'mouse.png', 'keyboard.png'];
  const per = dur / shots.length;
  return (
    <AbsoluteFill style={{ justifyContent: 'center', alignItems: 'center', opacity: fadeOut(frame, dur) }}>
      <div style={{ ...fadeUp(frame, 0, fps), position: 'absolute', top: 64, fontSize: 44, color: C.muted }}>
        Lighting, DPI, polling and profiles. All your devices, one screen.
      </div>
      <div style={{ position: 'absolute', bottom: 22, fontSize: 22, color: '#6b6d74' }}>Basilisk V3 is real hardware; the other devices are simulated.</div>
      {shots.map((s, i) => {
        const local = frame - i * per;
        const enter = spring({ frame: local, fps, config: { damping: 200 } });
        const leave = interpolate(local, [per - 10, per], [1, 0], { extrapolateLeft: 'clamp', extrapolateRight: 'clamp' });
        if (local < 0 || (local > per && i < shots.length - 1)) return null;
        return (
          <Img key={s} src={staticFile(s)} style={{
            position: 'absolute', top: 160, width: 1440, borderRadius: 18, border: `1px solid ${C.line}`,
            boxShadow: '0 40px 120px #000c', opacity: i < shots.length - 1 ? Math.min(enter, leave) : enter,
            transform: `translateY(${(1 - enter) * 60}px) scale(${0.96 + enter * 0.04})`,
          }} />
        );
      })}
    </AbsoluteFill>
  );
};

/** 5. Where to get it. */
const Outro: React.FC = () => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();
  return (
    <AbsoluteFill style={{ justifyContent: 'center', alignItems: 'center', textAlign: 'center' }}>
      <Img src={staticFile('logo.png')} style={{ ...fadeUp(frame, 0, fps), width: 150, height: 150, borderRadius: 34 }} />
      <div style={{ ...fadeUp(frame, 6, fps), fontSize: 92, fontWeight: 700, letterSpacing: -3, marginTop: 30 }}>Free and open source.</div>
      <div style={{ ...fadeUp(frame, 12, fps), fontSize: 50, color: C.accent, marginTop: 16, fontWeight: 600 }}>github.com/infinityInfinite/secbird</div>
      <div style={{ ...fadeUp(frame, 18, fps), fontSize: 32, color: C.muted, marginTop: 22 }}>Windows 10 / 11 · Beta · Not affiliated with Razer</div>
      <div style={{ ...fadeUp(frame, 30, fps), fontSize: 34, color: C.text, marginTop: 40, padding: '14px 28px', border: `1px solid ${C.line}`, borderRadius: 14, background: C.panel }}>
        Only tested on one mouse so far. Try it on yours and tell us how it went.
      </div>
    </AbsoluteFill>
  );
};

export const SCENES = { problems: 120, intro: 75, debloat: 120, popups: 50, light: 55, fast: 50, sticks: 55, screens: 165, outro: 120 };
export const TOTAL = Object.values(SCENES).reduce((a, b) => a + b, 0);

export const Promo: React.FC = () => {
  const S = SCENES;
  let t = 0;
  const at = (d: number) => { const from = t; t += d; return { from, durationInFrames: d }; };
  return (
    <AbsoluteFill style={{ background: `radial-gradient(1400px 800px at 50% 45%, #17120f, ${C.bg} 65%)`, color: C.text, fontFamily: FONT }}>
      <Sequence {...at(S.problems)}><Problems dur={S.problems} /></Sequence>
      <Sequence {...at(S.intro)}><Intro dur={S.intro} /></Sequence>
      <Sequence {...at(S.debloat)}><Fact big="0 background services" line="Turn Razer's services off. Your devices keep working." dur={S.debloat} extra={<Terminal />} /></Sequence>
      <Sequence {...at(S.popups)}><Fact big="0 pop-ups. 0 ads." line="No account, no updater, no internet." dur={S.popups} /></Sequence>
      <Sequence {...at(S.light)}><Fact big="650 KB" line="About 11 MB of RAM in the tray." dur={S.light} /></Sequence>
      <Sequence {...at(S.fast)}><Fact big="Found in a second" line="Plug in, and your settings are already on." dur={S.fast} /></Sequence>
      <Sequence {...at(S.sticks)}><Fact big="No restarts" line="Settings come back after replug, sleep and reboot." dur={S.sticks} /></Sequence>
      <Sequence {...at(S.screens)}><Screens dur={S.screens} /></Sequence>
      <Sequence {...at(S.outro)}><Outro /></Sequence>
    </AbsoluteFill>
  );
};
