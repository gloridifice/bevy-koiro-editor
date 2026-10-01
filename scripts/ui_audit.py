"""Measure a native Bevy snapshot and annotate its matching cutty screenshot.

Requires Pillow only for pixel evidence/annotation. No browser DOM is involved.
Usage: python scripts/ui_audit.py snapshot.json screenshot.png --output audit
Capture full-resolution screenshots at the same settled window size as the JSON.
Desktop target floor is 24px, not the 44px touch guideline. Balance is a signal,
not a gate: an editor's canvas is intentionally brighter than its tool panels.
"""
import argparse
import json
from pathlib import Path
from PIL import Image, ImageDraw


def bounds(node):
    x, y, w, h = node["rect"]
    return (x, y, x + w, y + h)


def intersection(a, b):
    return (max(a[0], b[0]), max(a[1], b[1]), min(a[2], b[2]), min(a[3], b[3]))


def area(rect):
    return max(0, rect[2] - rect[0]) * max(0, rect[3] - rect[1])


def luminance(rgb):
    linear = [c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4 for c in rgb[:3]]
    return sum(c * weight for c, weight in zip(linear, [0.2126, 0.7152, 0.0722]))


def audit(data, image):
    if len(data["windows"]) != 1:
        raise ValueError("Capture one audit window per process")
    window = data["windows"][0]
    width, height, scale = window["width"], window["height"], window["scale"]
    nodes = {n["id"]: n for n in data["nodes"]}
    viewport = (0, 0, width, height)

    def ancestors(node):
        while node["parent"] in nodes:
            node = nodes[node["parent"]]
            yield node

    def visible(node):
        result = intersection(bounds(node), viewport)
        for parent in ancestors(node):
            p = bounds(parent)
            result = intersection(result, (
                p[0] if parent["clip_x"] else -1e9, p[1] if parent["clip_y"] else -1e9,
                p[2] if parent["clip_x"] else 1e9, p[3] if parent["clip_y"] else 1e9))
        return result

    def unintended_clip(node, rect, shown):
        parents = list(ancestors(node))
        for axis, lo, hi in [("x", 0, 2), ("y", 1, 3)]:
            scroll_bounds = [bounds(p) for p in parents
                if p.get(f"scroll_{axis}", p.get("scroll") is not None and p[f"clip_{axis}"])]
            # Only the edge actually supplied by a scroll viewport is intentional.
            # A smaller non-scroll field/ancestor can still accidentally clip content.
            if rect[lo] < shown[lo] - 2 and not any(p[lo] >= shown[lo] - 2 for p in scroll_bounds):
                return True
            if rect[hi] > shown[hi] + 2 and not any(p[hi] <= shown[hi] + 2 for p in scroll_bounds):
                return True
        return False

    def layer(node):
        return max([node.get("z", 0), *[p.get("z", 0) for p in ancestors(node)]])

    def background(node):
        layers = [p.get("background") for p in [node, *ancestors(node)]]
        result = [20 / 255] * 3
        for color in reversed(layers):
            if color:
                result = [color[i] * color[3] + result[i] * (1 - color[3]) for i in range(3)]
        return result

    findings = {"clipped_controls": [], "clipped_text": [], "small_targets": [], "contrast": [], "collisions": []}
    blocks = []
    for node in nodes.values():
        rect, shown = bounds(node), visible(node)
        if node["interactive"]:
            if area(shown) < area(rect) * 0.98 and unintended_clip(node, rect, shown):
                findings["clipped_controls"].append(node["id"])
            if area(shown) > 0 and min(node["rect"][2:]) < 23:
                findings["small_targets"].append(node["id"])
        text = node["text"]
        if text and text.strip() and area(shown) > 0:
            if unintended_clip(node, rect, shown):
                findings["clipped_text"].append({"id": node["id"], "text": text})
            ink = node.get("ink")
            if ink:
                fg, bg = luminance(ink), luminance(background(node))
                ratio = (max(fg, bg) + 0.05) / (min(fg, bg) + 0.05)
                if ratio < 4.5:
                    findings["contrast"].append({"id": node["id"], "text": text, "ratio": round(ratio, 2)})
        if ((text and text.strip()) or node["input"] is not None) and area(shown) > 0:
            # Input hints deliberately occupy the same rectangle as their owner.
            if text and any(p["input"] is not None for p in ancestors(node)):
                continue
            blocks.append((node, shown))
    for index, (a, ra) in enumerate(blocks):
        for b, rb in blocks[index + 1:]:
            # Intentional overlay/underlay intersection is not a content collision.
            if layer(a) != layer(b):
                continue
            if any(p["id"] == a["id"] for p in ancestors(b)) or any(p["id"] == b["id"] for p in ancestors(a)):
                continue
            overlap = area(intersection(ra, rb))
            if overlap / min(area(ra), area(rb)) >= 0.12:
                findings["collisions"].append([a["id"], b["id"]])

    # Inspect every ordinary container, but do not mistake a flex spacer or wrap for
    # accidental unevenness. Edge insets and same-line gaps are actual rendered geometry.
    spacing = []
    for parent in nodes.values():
        children = [n for n in nodes.values() if n["parent"] == parent["id"]
                    and not n.get("absolute", False) and area(visible(n)) > 0]
        if not children or not parent.get("padding"):
            continue
        pr = bounds(parent)
        cb = [bounds(child) for child in children]
        insets = [min(r[0] for r in cb) - pr[0], min(r[1] for r in cb) - pr[1],
                  pr[2] - max(r[2] for r in cb), pr[3] - max(r[3] for r in cb)]
        direction = parent.get("flow", "Row")
        ordered = sorted(cb, key=lambda r: r[1] if direction == "Column" else r[0])
        gaps = []
        for a, b in zip(ordered, ordered[1:]):
            if direction == "Column":
                gaps.append(round(b[1] - a[3], 2))
            elif abs(a[1] - b[1]) < 2:
                gaps.append(round(b[0] - a[2], 2))
        spacing.append({"id": parent["id"], "padding": parent["padding"],
            "rendered_edge_insets": [round(n, 2) for n in insets], "gaps": gaps,
            "labels": [n["text"] for n in children if n["text"]]})

    # Independent pixel oracle. Ignore OS chrome; ink weight is RGB distance to #202020.
    left = (image.width - width * scale) / 2
    top = image.height - height * scale - left
    if not (0 <= left <= 32 * scale and 0 <= top <= 128 * scale):
        raise ValueError("Screenshot must be full resolution and match the snapshot window/DPI")
    client = image.crop((round(left), round(top), round(left + width * scale), round(top + height * scale)))
    pixels = client.convert("RGB").resize((max(1, round(width)), max(1, round(height))))
    total = moment_x = moment_y = left_weight = top_weight = 0
    for y in range(pixels.height):
        for x in range(pixels.width):
            rgb = pixels.getpixel((x, y))
            weight = sum(abs(c - 32) for c in rgb) / 3
            total += weight
            moment_x += weight * (x + 0.5)
            moment_y += weight * (y + 0.5)
            if x < pixels.width / 2: left_weight += weight
            if y < pixels.height / 2: top_weight += weight
    centroid = [moment_x / total / pixels.width, moment_y / total / pixels.height]
    metrics = {"window": window, "counts": {key: len(values) for key, values in findings.items()},
        "findings": findings, "container_spacing": spacing, "pixel_balance_signal": {"centroid": [round(c, 4) for c in centroid],
            "optical_offset": [round(centroid[0] - 0.50, 4), round(centroid[1] - 0.46, 4)],
            "left_right_imbalance": round(abs(total - 2 * left_weight) / total, 4),
            "top_bottom_imbalance": round(abs(total - 2 * top_weight) / total, 4)}}
    draw = ImageDraw.Draw(image)
    def box(entity, color):
        x0, y0, x1, y1 = bounds(nodes[entity])
        draw.rectangle((left + x0 * scale, top + y0 * scale, left + x1 * scale, top + y1 * scale), outline=color, width=3)
    for entity in findings["clipped_controls"]: box(entity, "red")
    for item in findings["contrast"]: box(item["id"], "orange")
    for a, b in findings["collisions"]: box(a, "magenta"); box(b, "magenta")
    cx, cy = left + centroid[0] * width * scale, top + centroid[1] * height * scale
    draw.ellipse((cx - 8, cy - 8, cx + 8, cy + 8), fill="cyan")
    draw.line((left + width * scale / 2, top, left + width * scale / 2, top + height * scale), fill="cyan", width=1)
    draw.text((left + 10, top + 10), str(metrics["counts"]), fill="cyan", stroke_width=1, stroke_fill="black")
    return metrics


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("snapshot", type=Path)
    parser.add_argument("screenshot", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    data = json.loads(args.snapshot.read_text(encoding="utf-8"))
    image = Image.open(args.screenshot).convert("RGB")
    result = audit(data, image)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.with_suffix(".json").write_text(json.dumps(result, indent=2), encoding="utf-8")
    image.save(args.output.with_suffix(".png"))
    print(json.dumps({"counts": result["counts"], "balance": result["pixel_balance_signal"]}))
