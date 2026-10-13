import React from 'react';
import {
  AbsoluteFill, Audio, Img, Sequence, interpolate, spring, staticFile, useCurrentFrame, useVideoConfig, Easing,
} from 'remotion';
import { C, FONT, fadeUp, fadeOut } from './Promo';

/**
 * Vertical 1080x1920 cut for YouTube Shorts. Content stays out of the bottom ~380 px and the
 * right ~140 px, where Shorts draws the title, channel name and like/comment buttons.
 */
const SAFE: React.CSSProperties = { position: 'absolute', left: 60, right: 140, top: 140, bottom: 380, display: 'flex', flexDirection: 'column', justifyContent: 'center', alignItems: 'center', textAlign: 'center' };

const Problems: React.FC<{ dur: number }> = ({ dur }) => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();
  const items = ['Background services', 'Pop-ups and ads', '"Restart to finish setup"', '~200 MB all day'];
  return (
    <AbsoluteFill style={{ opacity: fadeOut(frame, dur) }}>
      <div style={{ ...SAFE, alignItems: 'flex-start', textAlign: 'left' }}>
        <div style={{ ...fadeUp(frame, 0, fps), color: C.muted, fontSize: 46, marginBottom: 34, lineHeight: 1.25 }}>Your mouse needs a driver.<br />Not this:</div>
        {items.map((t, i) => {
          const start = 8 + i * 10;
          const strike = interpolate(frame, [start + 26, start + 38], [0, 100], { extrapolateLeft: 'clamp', extrapolateRight: 'clamp', easing: Easing.out(Easing.cubic) });
          return (
            <div key={t} style={{ ...fadeUp(frame, start, fps), position: 'relative', fontSize: 66, fontWeight: 700, margin: '10px 0', color: strike > 50 ? C.muted : C.text, letterSpacing: -1.5, lineHeight: 1.15 }}>
              {t}
              <div style={{ position: 'absolute', left: -8, top: '52%', height: 7, borderRadius: 4, background: C.accent, width: `calc(${strike}% + 16px)` }} />
            </div>
          );
        })}
      </div>
    </AbsoluteFill>
  );
};

const Intro: React.FC<{ dur: number }> = ({ dur }) => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();
  const pop = spring({ frame, fps, config: { damping: 14, mass: 0.8 } });
  return (
    <AbsoluteFill style={{ opacity: fadeOut(frame, dur) }}>
      <div style={SAFE}>
        <Img src={staticFile('logo.png')} style={{ width: 280, height: 280, borderRadius: 64, transform: `scale(${pop})`, boxShadow: '0 30px 80px #0009' }} />
        <div style={{ ...fadeUp(frame, 8, fps), fontSize: 116, fontWeight: 700, letterSpacing: -4, lineHeight: 1, marginTop: 56 }}>Secretarybird</div>
        <div style={{ ...fadeUp(frame, 18, fps), fontSize: 54, color: C.muted, marginTop: 26, lineHeight: 1.25 }}>
          Your Razer gear,<br /><span style={{ color: C.accent, fontWeight: 600 }}>minus the bloat.</span>
        </div>
      </div>
    </AbsoluteFill>
  );
};

const Fact: React.FC<{ big: React.ReactNode; line: string; dur: number; extra?: React.ReactNode }> = ({ big, line, dur, extra }) => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();
  return (
    <AbsoluteFill style={{ opacity: fadeOut(frame, dur, 8) }}>
      <div style={SAFE}>
        <div style={{ ...fadeUp(frame, 0, fps), fontSize: 132, fontWeight: 750, letterSpacing: -5, lineHeight: 1.02 }}>{big}</div>
        <div style={{ ...fadeUp(frame, 6, fps), fontSize: 48, color: C.muted, marginTop: 30, lineHeight: 1.3 }}>{line}</div>
        {extra && <div style={{ ...fadeUp(frame, 14, fps), marginTop: 50, alignSelf: 'stretch' }}>{extra}</div>}
      </div>
    </AbsoluteFill>
  );
};

const Terminal: React.FC = () => {
  const frame = useCurrentFrame();
  const lines = [
    ['$ ', 'secbird debloat apply', C.text],
    ['', 'Razer Elevation Service', C.muted],
    ['  ', 'running -> stopped', C.text],
    ['', 'Razer Game Manager Service', C.muted],
    ['  ', 'running -> stopped', C.text],
    ['', 'Undo any time: secbird debloat undo', C.ok],
  ] as const;
  return (
    <div style={{ textAlign: 'left', background: C.panel, border: `1px solid ${C.line}`, borderRadius: 20, padding: '26px 30px', font: '30px/1.55 Consolas, monospace' }}>
      {lines.map(([p, t, color], i) => {
        const shown = Math.max(0, Math.min(t.length, Math.floor((frame - 14 - i * 8) * (i === 0 ? 1.4 : 6))));
        return <div key={i} style={{ color, whiteSpace: 'pre', minHeight: 46 }}>{shown > 0 ? p + t.slice(0, shown) : ''}</div>;
      })}
    </div>
  );
};

/** Each screenshot fills the width and slowly pans, so the 16:10 shots read on a phone. */
const Screens: React.FC<{ dur: number }> = ({ dur }) => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();
  const shots = ['overview.png', 'mouse.png', 'keyboard.png'];
  const per = dur / shots.length;
  return (
    <AbsoluteFill style={{ opacity: fadeOut(frame, dur) }}>
      <div style={{ ...fadeUp(frame, 0, fps), position: 'absolute', top: 210, left: 60, right: 140, fontSize: 54, fontWeight: 650, lineHeight: 1.2, textAlign: 'center' }}>
        Lighting, DPI, polling, profiles.<br /><span style={{ color: C.accent }}>All devices, one screen.</span>
      </div>
      {shots.map((s, i) => {
        const local = frame - i * per;
        if (local < 0 || (local > per && i < shots.length - 1)) return null;
        const enter = spring({ frame: local, fps, config: { damping: 200 } });
        const leave = i < shots.length - 1 ? interpolate(local, [per - 8, per], [1, 0], { extrapolateLeft: 'clamp', extrapolateRight: 'clamp' }) : 1;
        const pan = interpolate(local, [0, per], [0, -260], { extrapolateRight: 'clamp' });
        return (
          <div key={s} style={{ position: 'absolute', top: 470, left: 40, width: 1000, height: 940, overflow: 'hidden', borderRadius: 24, border: `1px solid ${C.line}`, boxShadow: '0 40px 120px #000c', opacity: Math.min(enter, leave), transform: `translateY(${(1 - enter) * 50}px)` }}>
            <Img src={staticFile(s)} style={{ height: 940, width: 'auto', transform: `translateX(${pan}px)` }} />
          </div>
        );
      })}
      <div style={{ position: 'absolute', top: 1430, left: 60, right: 140, fontSize: 26, color: '#6b6d74', textAlign: 'center' }}>Basilisk V3 is real hardware; the others are simulated.</div>
    </AbsoluteFill>
  );
};

const Outro: React.FC = () => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();
  return (
    <AbsoluteFill>
      <div style={SAFE}>
        <Img src={staticFile('logo.png')} style={{ ...fadeUp(frame, 0, fps), width: 190, height: 190, borderRadius: 44 }} />
        <div style={{ ...fadeUp(frame, 6, fps), fontSize: 92, fontWeight: 700, letterSpacing: -3, marginTop: 36, lineHeight: 1.05 }}>Free and<br />open source.</div>
        <div style={{ ...fadeUp(frame, 12, fps), fontSize: 50, color: C.accent, marginTop: 30, fontWeight: 600 }}>github.com/infinityInfinite/secbird</div>
        <div style={{ ...fadeUp(frame, 18, fps), fontSize: 34, color: C.muted, marginTop: 22 }}>Windows 10 / 11 · Beta · Not affiliated with Razer</div>
        <div style={{ ...fadeUp(frame, 28, fps), fontSize: 38, color: C.text, marginTop: 44, padding: '18px 28px', border: `1px solid ${C.line}`, borderRadius: 18, background: C.panel, lineHeight: 1.3 }}>
          Only tested on one mouse so far.<br />Try it on yours and tell us how it went.
        </div>
      </div>
    </AbsoluteFill>
  );
};

export const SHORT_SCENES = { problems: 105, intro: 70, debloat: 120, popups: 50, light: 55, sticks: 55, screens: 180, outro: 135 };
export const SHORT_TOTAL = Object.values(SHORT_SCENES).reduce((a, b) => a + b, 0);

export const Short: React.FC = () => {
  const S = SHORT_SCENES;
  let t = 0;
  const at = (d: number) => { const from = t; t += d; return { from, durationInFrames: d }; };
  return (
    <AbsoluteFill style={{ background: `radial-gradient(1100px 1500px at 50% 42%, #17120f, ${C.bg} 65%)`, color: C.text, fontFamily: FONT }}>
      {/* Original track from scripts/music.py; its drop lands on the logo at 3.5 s. */}
      <Audio src={staticFile('music.mp3')} />
      <Sequence {...at(S.problems)}><Problems dur={S.problems} /></Sequence>
      <Sequence {...at(S.intro)}><Intro dur={S.intro} /></Sequence>
      <Sequence {...at(S.debloat)}><Fact big={<>0 background<br />services</>} line="Turn Razer's services off. Your devices keep working." dur={S.debloat} extra={<Terminal />} /></Sequence>
      <Sequence {...at(S.popups)}><Fact big={<>0 pop-ups.<br />0 ads.</>} line="No account, no updater, no internet." dur={S.popups} /></Sequence>
      <Sequence {...at(S.light)}><Fact big="650 KB" line="About 11 MB of RAM in the tray." dur={S.light} /></Sequence>
      <Sequence {...at(S.sticks)}><Fact big={<>No<br />restarts</>} line="Settings come back after replug, sleep and reboot." dur={S.sticks} /></Sequence>
      <Sequence {...at(S.screens)}><Screens dur={S.screens} /></Sequence>
      <Sequence {...at(S.outro)}><Outro /></Sequence>
    </AbsoluteFill>
  );
};
