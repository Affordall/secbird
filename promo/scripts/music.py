"""Synthesizes the Short's background track (original, so no copyright claims).

137.14 BPM puts two bars exactly at 3.5 s, where the logo appears, so the beat drops on the cut.
Usage: python scripts/music.py public/music.wav [seconds]
"""
import sys
import wave

import numpy as np
from scipy.signal import butter, sosfilt

SR = 44100
BEAT = 3.5 / 8
BAR = BEAT * 4
LENGTH = float(sys.argv[2]) if len(sys.argv) > 2 else 25.7
OUTRO = 21.17  # where the outro scene starts: drums drop out
DROP = 3.5

N = int(LENGTH * SR)
t_all = np.arange(N) / SR
rng = np.random.default_rng(7)


def midi(n):
    return 440.0 * 2 ** ((n - 69) / 12)


def lowpass(x, hz, order=2):
    return sosfilt(butter(order, hz, 'low', fs=SR, output='sos'), x)


def highpass(x, hz, order=2):
    return sosfilt(butter(order, hz, 'high', fs=SR, output='sos'), x)


def saw(f, t, detune=(0.0,)):
    return sum(2 * ((t * f * (1 + d)) % 1) - 1 for d in detune) / len(detune)


def env(n, a, r):
    """Linear attack, exponential release, in seconds."""
    t = np.arange(n) / SR
    return np.minimum(1, t / max(a, 1e-4)) * np.exp(-t / r)


def place(buf, sig, start):
    i = int(start * SR)
    if i >= len(buf):
        return
    sig = sig[: len(buf) - i]
    buf[i: i + len(sig)] += sig


# A minor: Am F C G, one chord per bar.
CHORDS = [[57, 60, 64], [53, 57, 60], [48, 52, 55], [55, 59, 62]]
ROOTS = [45, 41, 48, 43]

pad = np.zeros(N); bass = np.zeros(N); arp = np.zeros(N)
kick = np.zeros(N); hats = np.zeros(N); fx = np.zeros(N)

bars = int(np.ceil(LENGTH / BAR))
for b in range(bars):
    start = b * BAR
    chord, root = CHORDS[b % 4], ROOTS[b % 4]
    n = int((BAR + 0.4) * SR)
    t = np.arange(n) / SR
    e = np.minimum(1, t / 0.25) * np.minimum(1, np.maximum(0, (BAR + 0.4 - t) / 0.4))
    p = sum(saw(midi(m), t, (-0.004, 0.0, 0.005)) for m in chord) / 3
    place(pad, p * e, start)

    for k in range(8):  # offbeat-heavy 8th-note bass
        st = start + k * BEAT / 2
        if st < DROP or st >= OUTRO:
            continue
        nn = int(BEAT / 2 * SR)
        tt = np.arange(nn) / SR
        f = midi(root - 12 if k % 2 == 0 else root)
        s = 0.6 * saw(f, tt) + 0.4 * np.sin(2 * np.pi * f * tt)
        place(bass, s * env(nn, 0.004, 0.12), st)

    pattern = [0, 1, 2, 1, 0, 2, 1, 2] * 2  # 16th-note pluck arpeggio over the chord
    for k, idx in enumerate(pattern):
        st = start + k * BEAT / 4
        if st < 1.75:
            continue
        nn = int(0.3 * SR)
        tt = np.arange(nn) / SR
        f = midi(chord[idx] + 12)
        s = saw(f, tt, (-0.003, 0.003)) * env(nn, 0.002, 0.07)
        place(arp, s, st)

for k in range(int(LENGTH / BEAT) + 1):
    st = k * BEAT
    if DROP <= st < OUTRO:  # four-on-the-floor kick
        nn = int(0.35 * SR)
        tt = np.arange(nn) / SR
        f = 45 + 110 * np.exp(-tt / 0.03)
        place(kick, np.sin(2 * np.pi * np.cumsum(f) / SR) * env(nn, 0.001, 0.12), st)
    off = st + BEAT / 2  # offbeat hats; quiet ticks before the drop
    if off < OUTRO:
        nn = int(0.09 * SR)
        place(hats, rng.standard_normal(nn) * env(nn, 0.001, 0.025) * (1.0 if off >= DROP else 0.35), off)

# Noise riser into the drop, and a soft impact on it.
rise_len = 1.75
nn = int(rise_len * SR)
tt = np.arange(nn) / SR
place(fx, highpass(rng.standard_normal(nn), 2000) * (tt / rise_len) ** 2 * 0.5, DROP - rise_len)
nn = int(1.2 * SR)
place(fx, lowpass(rng.standard_normal(nn), 900) * env(nn, 0.002, 0.35) * 0.8, DROP)

# Sidechain: duck pad and bass on every kick.
duck = np.ones(N)
for k in range(int(LENGTH / BEAT) + 1):
    st = k * BEAT
    if DROP <= st < OUTRO:
        i = int(st * SR)
        nn = min(int(BEAT * SR), N - i)
        duck[i: i + nn] = 1 - 0.65 * np.exp(-np.arange(nn) / SR / 0.09)

# Pad filter opens at the drop.
pad_dark = lowpass(pad, 700)
pad_open = lowpass(pad, 2600)
mix_amt = np.clip((t_all - (DROP - 0.3)) / 0.3, 0, 1)
pad = pad_dark * (1 - mix_amt) + pad_open * mix_amt

# Pad louder where there are no drums, so the intro and outro aren't near-silent on a phone.
pad_gain = 1 + 1.6 * (1 - mix_amt) + 1.2 * np.clip((t_all - OUTRO) / 0.3, 0, 1)

mix = (0.22 * pad * pad_gain * duck + 0.30 * lowpass(bass, 900) * duck + 0.10 * lowpass(arp, 3500)
       + 0.55 * kick + 0.10 * highpass(hats, 7000) + fx * 0.25)

# Stereo: arp and hats slightly wider via a short delay on one side.
d = int(0.012 * SR)
wide = 0.10 * lowpass(arp, 3500) + 0.10 * highpass(hats, 7000)
left = mix
right = mix - wide + np.concatenate([np.zeros(d), wide[:-d]])

fade = np.clip((LENGTH - t_all) / 2.5, 0, 1) * np.clip(t_all / 0.05, 0, 1)
stereo = np.stack([left, right], axis=1) * fade[:, None]
stereo = np.tanh(stereo / np.abs(stereo).max() * 1.4) * 0.85  # gentle saturation, -1.4 dBFS peak

with wave.open(sys.argv[1], 'wb') as w:
    w.setnchannels(2)
    w.setsampwidth(2)
    w.setframerate(SR)
    w.writeframes((stereo * 32767).astype('<i2').tobytes())
print(f'wrote {sys.argv[1]} ({LENGTH:.2f} s)')
