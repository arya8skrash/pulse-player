#!/usr/bin/env python3
"""Fetch real AutoEQ ParametricEQ.txt files and emit a Rust PROFILES array.

Data sources: jaakkopasanen/AutoEq, results/oratory1990/* and results/crinacle/*.
Nothing is invented: a device with no confident match is reported and omitted.

Usage:  python generate_profiles.py [--out profiles_generated.rs]
Set GITHUB_TOKEN to avoid API rate limits (optional; only a few API calls are made).
"""
import argparse
import os
import re
import sys
from difflib import SequenceMatcher
from urllib.parse import quote

import requests

REPO = "jaakkopasanen/AutoEq"
BRANCH = "master"
API = f"https://api.github.com/repos/{REPO}/git/trees"
RAW = f"https://raw.githubusercontent.com/{REPO}/{BRANCH}"
SOURCES = ["oratory1990", "crinacle"]  # priority order: oratory1990 wins on ties
FUZZY_CUTOFF = 0.90

# (query name, [aliases as AutoEQ may spell them])
HEADPHONES = [
    ("Sony WH-1000XM5", []), ("Sony WH-1000XM4", []), ("Apple AirPods Max", []),
    ("Bose QuietComfort Ultra Headphones", ["Bose QuietComfort Ultra"]),
    ("Sennheiser HD 600", []), ("Sennheiser HD 650", []), ("Sennheiser HD 800 S", ["Sennheiser HD800S"]),
    ("Beyerdynamic DT 770 PRO (250 Ohm)", ["Beyerdynamic DT 770 Pro 250 Ohm"]),
    ("Beyerdynamic DT 990 PRO", ["Beyerdynamic DT 990 Pro 250 Ohm"]),
    ("Audio-Technica ATH-M50x", []), ("HiFiMan Sundara", []), ("AKG K371", []), ("AKG K702", []),
    ("Focal Clear Mg", []), ("Meze 109 Pro", []), ("Philips Fidelio X2HR", []),
    ("Razer BlackShark V2 Pro", []), ("SteelSeries Arctis Nova Pro", []),
    ("Audeze LCD-5", []), ("Dan Clark Audio Stealth", []),
]
EARBUDS = [
    ("Apple AirPods Pro (2nd Gen)", ["Apple AirPods Pro 2"]),
    ("Apple AirPods Pro (1st Gen)", ["Apple AirPods Pro"]),
    ("Samsung Galaxy Buds2 Pro", []), ("Sony WF-1000XM5", []), ("Sony WF-1000XM4", []),
    ("Technics EAH-AZ80", []), ("Sennheiser Momentum True Wireless 4", []),
    ("Bose QuietComfort Ultra Earbuds", []), ("Google Pixel Buds Pro", []),
    ("Nothing Ear (a)", ["Nothing Ear a"]), ("OnePlus Buds Pro 2", []),
    ("Anker Soundcore Liberty 4 NC", ["Soundcore Liberty 4 NC"]),
    ("Moondrop Aria", []), ("Moondrop Blessing 3", []), ("7Hz Salnotes Zero", ["Salnotes Zero"]),
    ("Linsoul 7Hz Timeless", ["7Hz Timeless"]),
    ("Truthear x Crinacle Zero:RED", ["Truthear Zero Red", "Truthear x Crinacle Zero Red"]),
    ("Dunu Titan S", []), ("Final Audio A4000", ["Final A4000"]), ("Beats Fit Pro", []),
]
# AutoEQ only measures headphones/IEMs. It has no speaker data, so these cannot be generated here.
SPEAKERS = [
    "Genelec 8030C", "Neumann KH 120 II", "Adam Audio A7V", "Focal Alpha 65 Evo", "JBL 305P MkII",
    "Kali Audio LP-6 V2", "KRK Rokit RP5 G4", "PreSonus Eris E3.5", "Audioengine A2+", "Edifier MR4",
    "Edifier R1280T", "Creative Pebble Pro", "Logitech Z407", "Razer Nommo Chroma", "KEF LSX II",
    "Q Acoustics M20 HD", "iLoud Micro Monitor", "Yamaha HS5", "Mackie CR3-X", "JBL 104-BT",
]

TYPE_MAP = {"PK": "Peaking", "LSC": "LowShelf", "LS": "LowShelf", "HSC": "HighShelf", "HS": "HighShelf"}
FILTER_RE = re.compile(
    r"Filter\s+\d+:\s+ON\s+(PK|LSC|HSC|LS|HS)\s+Fc\s+([\d.]+)\s+Hz\s+Gain\s+(-?[\d.]+)\s+dB(?:\s+Q\s+([\d.]+))?",
    re.I,
)
PREAMP_RE = re.compile(r"Preamp:\s*(-?[\d.]+)\s*dB", re.I)

session = requests.Session()
if os.environ.get("GITHUB_TOKEN"):
    session.headers["Authorization"] = f"Bearer {os.environ['GITHUB_TOKEN']}"
session.headers["Accept"] = "application/vnd.github+json"


def norm(s):
    return " ".join(re.sub(r"[^a-z0-9]+", " ", s.lower()).split())


def digit_tokens(s):
    return {t for t in norm(s).split() if any(c.isdigit() for c in t)}


def tree(path):
    """Non-recursive git tree listing; avoids the truncation of the recursive API."""
    r = session.get(f"{API}/{BRANCH}:{quote(path)}", timeout=60)
    r.raise_for_status()
    return r.json()["tree"]


def build_index():
    """Returns list of (source, form_dir, device_name) for every device folder."""
    index = []
    for src in SOURCES:
        for form in tree(f"results/{src}"):
            if form["type"] != "tree":
                continue
            fname = form["path"]
            print(f"  indexing {src}/{fname}", file=sys.stderr)
            for dev in tree(f"results/{src}/{fname}"):
                if dev["type"] == "tree":
                    index.append((src, fname, dev["path"]))
    return index


def form_ok(form_dir, category):
    f = form_dir.lower()
    return ("in-ear" in f) if category == "Earbud" else ("over-ear" in f)


def match(index, query, aliases, category):
    names = [query] + aliases
    best = None  # (exact, score, -rank, entry)
    for entry in index:
        src, form, dev = entry
        if not form_ok(form, category):
            continue
        for n in names:
            exact = norm(n) == norm(dev)
            score = 1.0 if exact else SequenceMatcher(None, norm(n), norm(dev)).ratio()
            if not exact and (score < FUZZY_CUTOFF or digit_tokens(n) != digit_tokens(dev)):
                continue
            key = (exact, score, -SOURCES.index(src))
            if best is None or key > best[:3]:
                best = (*key, entry)
    return best[3] if best else None


def fetch_peq(src, form, dev):
    path = f"results/{src}/{form}/{dev}/{dev} ParametricEQ.txt"
    r = session.get(f"{RAW}/{quote(path)}", timeout=60)
    r.raise_for_status()
    return r.text


def parse_peq(text):
    m = PREAMP_RE.search(text)
    preamp = float(m.group(1)) if m else 0.0
    bands = []
    for t, fc, g, q in FILTER_RE.findall(text):
        default_q = 0.7 if t.upper() != "PK" else 1.0
        bands.append((TYPE_MAP[t.upper()], float(fc), float(g), float(q) if q else default_q))
    return preamp, bands


def rust_str(s):
    return s.replace("\\", "\\\\").replace('"', '\\"')


def emit(profiles):
    out = ["pub static PROFILES: &[HeadphoneProfile] = &["]
    for name, cat, src, form, preamp, bands in profiles:
        out.append("    HeadphoneProfile {")
        out.append(f'        name: "{rust_str(name)}", // AutoEQ: {src}/{form}')
        out.append(f"        category: Category::{cat},")
        out.append(f"        preamp_db: {preamp:.1f},")
        out.append("        bands: &[")
        for t, fc, g, q in bands:
            out.append(
                f"            EqBand {{ filter_type: FilterType::{t}, freq: {fc:.1f}, gain_db: {g:.1f}, q: {q:.2f} }},"
            )
        out.append("        ],")
        out.append("    },")
    out.append("];")
    return "\n".join(out) + "\n"


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", default="profiles_generated.rs")
    args = ap.parse_args()

    print("Indexing AutoEQ...", file=sys.stderr)
    index = build_index()

    profiles, missing = [], []
    for category, devices in (("Headphone", HEADPHONES), ("Earbud", EARBUDS)):
        for name, aliases in devices:
            hit = match(index, name, aliases, category)
            if not hit:
                missing.append((name, "no confident match in AutoEQ"))
                continue
            try:
                preamp, bands = parse_peq(fetch_peq(*hit))
            except requests.RequestException as e:
                missing.append((name, f"download failed: {e}"))
                continue
            if not bands:
                missing.append((name, "ParametricEQ.txt had no parsable filters"))
                continue
            print(f"OK   {name}  <-  {hit[0]}/{hit[1]}/{hit[2]}", file=sys.stderr)
            profiles.append((name, category, hit[0], hit[1], preamp, bands))

    for name in SPEAKERS:
        missing.append((name, "speaker: AutoEQ has no speaker measurements"))

    with open(args.out, "w", encoding="utf-8") as f:
        f.write(emit(profiles))

    print(f"\nWrote {len(profiles)} profiles to {args.out}", file=sys.stderr)
    print(f"Skipped {len(missing)}:", file=sys.stderr)
    for name, why in missing:
        print(f"  - {name}: {why}", file=sys.stderr)


if __name__ == "__main__":
    main()
