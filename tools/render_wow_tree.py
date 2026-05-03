#!/usr/bin/env python3
"""Render the WoW completion tree as a static HTML dashboard.

The TOML file is the editable source of truth. Parent nodes are inferred from
dot-separated IDs and their color/completion roll up from leaf statuses.
"""

from __future__ import annotations

import argparse
import html
import json
import re
import sys
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

try:
    import tomllib
except ModuleNotFoundError:  # pragma: no cover - Python < 3.11 fallback.
    tomllib = None


STATUS_SCORE = {"red": 0.0, "yellow": 0.5, "green": 1.0}
STATUS_LABEL = {"red": "Red", "yellow": "Yellow", "green": "Green"}

SYSTEM_LABELS = {
    "WOW": "WoW Classic",
    "WOW.AUTH": "Account, Auth, And Realm",
    "WOW.PROTOCOL": "World Session And Protocol",
    "WOW.CHARACTER": "Character Lifecycle",
    "WOW.WORLD": "World Runtime, Maps, And Visibility",
    "WOW.MOVEMENT": "Movement, Navigation, And Physics",
    "WOW.OBJECTS": "Creatures, NPCs, And Gameobjects",
    "WOW.COMBAT": "Combat",
    "WOW.SPELLS": "Spells, Auras, Classes, And Resources",
    "WOW.QUESTS": "Quests And Objectives",
    "WOW.ITEMS": "Loot, Inventory, Items, And Economy",
    "WOW.PROGRESSION": "Progression, Stats, Skills, And Reputation",
    "WOW.PERSISTENCE": "Persistence And Relog Durability",
    "WOW.SOCIAL": "Social, Group, Guild, And Communication",
    "WOW.INSTANCES": "Dungeons, Instances, Raids, And Encounters",
    "WOW.PVP": "PvP, Honor, Battlegrounds, And World PvP",
    "WOW.SCRIPTS": "AI, Scripts, Events, And World Logic",
    "WOW.DATA": "Data Fidelity And Content Loading",
    "WOW.TOOLING": "Operations, Tooling, And Test Harnesses",
}

CP2_LEAVES = {
    "WOW.QUESTS.ELIGIBILITY.LEVEL",
    "WOW.QUESTS.ELIGIBILITY.RACE_CLASS",
    "WOW.QUESTS.ELIGIBILITY.PREREQUISITE_CHAIN",
    "WOW.QUESTS.ELIGIBILITY.REPEATABLE_DAILY",
    "WOW.QUESTS.OBJECTIVES.ITEM_DROPS",
    "WOW.ITEMS.LOOT.QUEST_ITEMS",
    "WOW.DATA.WORLD_DB_LOOT",
    "WOW.OBJECTS.GAMEOBJECT.QUEST_USE",
    "WOW.QUESTS.OBJECTIVES.GAMEOBJECT",
    "WOW.SPELLS.GCD.GLOBAL",
    "WOW.SPELLS.COOLDOWN.PER_SPELL",
    "WOW.SPELLS.WARRIOR.HEROIC_STRIKE",
    "WOW.SPELLS.WARRIOR.BATTLE_SHOUT",
    "WOW.SPELLS.WARRIOR.LEVEL_1_6",
    "WOW.COMBAT.LOG.MELEE",
    "WOW.COMBAT.LOG.SPELL",
    "WOW.SPELLS.RESOURCES.HEALTH_REGEN",
    "WOW.SPELLS.RESOURCES.RAGE_DECAY",
    "WOW.PERSISTENCE.HEALTH_POWER",
    "WOW.PROGRESSION.SKILLS.LOAD_SHOW",
    "WOW.PROGRESSION.SKILLS.WEAPON_ADVANCE",
    "WOW.PERSISTENCE.SPELLS_SKILLS",
    "WOW.COMBAT.AGGRO.ON_SIGHT",
    "WOW.COMBAT.AGGRO.ASSIST",
    "WOW.COMBAT.CHASE.MOVE_INTO_RANGE",
    "WOW.COMBAT.LEASH.EVADE_HOME",
    "WOW.MOVEMENT.PATHFINDING.MMAP",
    "WOW.MOVEMENT.CREATURE.PATROL_LONG_RUNNING",
    "WOW.MOVEMENT.CREATURE.RANDOM",
    "WOW.MOVEMENT.CREATURE.WAYPOINT",
    "WOW.WORLD.GRID.IDLE_UNLOAD",
}


@dataclass
class Node:
    id: str
    label: str
    requirement: str = ""
    proof: str = ""
    status: str | None = None
    required_for: list[str] = field(default_factory=list)
    children: dict[str, "Node"] = field(default_factory=dict)
    leaf_count: int = 0
    red_count: int = 0
    yellow_count: int = 0
    green_count: int = 0
    completion: float = 0.0
    derived_status: str = "red"

    def to_dict(self) -> dict[str, Any]:
        return {
            "id": self.id,
            "label": self.label,
            "requirement": self.requirement,
            "proof": self.proof,
            "status": self.derived_status,
            "rawStatus": self.status,
            "requiredFor": self.required_for,
            "leafCount": self.leaf_count,
            "redCount": self.red_count,
            "yellowCount": self.yellow_count,
            "greenCount": self.green_count,
            "completion": self.completion,
            "children": [child.to_dict() for child in sorted_children(self)],
        }


def sorted_children(node: Node) -> list[Node]:
    return sorted(node.children.values(), key=lambda child: (child.id.count("."), child.id))


def repo_root() -> Path:
    return Path(__file__).resolve().parents[1]


def title_from_segment(segment: str) -> str:
    known = {
        "AUTH": "Auth",
        "SRP": "SRP",
        "DB": "DB",
        "DBC": "DBC",
        "NPC": "NPC",
        "NPCS": "NPCs",
        "PVP": "PvP",
        "AI": "AI",
        "LOS": "LOS",
        "XP": "XP",
        "GCD": "GCD",
        "CP2": "CP2",
        "MMAP": "MMap",
        "VMAP": "VMap",
    }
    return " ".join(known.get(part, part.title()) for part in segment.split("_"))


def label_for_id(node_id: str) -> str:
    return SYSTEM_LABELS.get(node_id, title_from_segment(node_id.rsplit(".", 1)[-1]))


def normalize_status(status: str | None) -> str | None:
    if status is None:
        return None
    normalized = status.strip().lower()
    if normalized not in STATUS_SCORE:
        raise ValueError(f"unknown status {status!r}; expected red, yellow, or green")
    return normalized


def load_toml(path: Path) -> list[dict[str, Any]]:
    if tomllib is None:
        raise RuntimeError("Python 3.11+ is required for built-in TOML parsing")
    with path.open("rb") as handle:
        data = tomllib.load(handle)
    nodes = data.get("node", [])
    if not isinstance(nodes, list):
        raise ValueError("expected [[node]] entries in TOML")
    return nodes


def build_tree(rows: list[dict[str, Any]]) -> Node:
    root = Node(id="WOW", label=SYSTEM_LABELS["WOW"])
    by_id = {"WOW": root}

    def ensure_node(node_id: str) -> Node:
        if node_id in by_id:
            return by_id[node_id]
        parent_id = node_id.rsplit(".", 1)[0]
        parent = ensure_node(parent_id)
        node = Node(id=node_id, label=label_for_id(node_id))
        parent.children[node_id] = node
        by_id[node_id] = node
        return node

    for row in rows:
        node_id = str(row["id"]).strip()
        if not node_id.startswith("WOW"):
            raise ValueError(f"node id must start with WOW: {node_id}")
        node = ensure_node(node_id)
        node.label = str(row.get("label") or label_for_id(node_id)).strip()
        node.requirement = str(row.get("requirement") or "").strip()
        node.proof = str(row.get("proof") or "").strip()
        node.status = normalize_status(row.get("status"))
        required_for = row.get("required_for", [])
        if node_id in CP2_LEAVES and "CP2" not in required_for:
            required_for = [*required_for, "CP2"]
        node.required_for = sorted({str(tag) for tag in required_for})

    roll_up(root)
    return root


def roll_up(node: Node) -> None:
    for child in node.children.values():
        roll_up(child)

    if not node.children:
        status = normalize_status(node.status) or "red"
        node.derived_status = status
        node.leaf_count = 1
        node.red_count = 1 if status == "red" else 0
        node.yellow_count = 1 if status == "yellow" else 0
        node.green_count = 1 if status == "green" else 0
        node.completion = STATUS_SCORE[status]
        return

    node.leaf_count = sum(child.leaf_count for child in node.children.values())
    node.red_count = sum(child.red_count for child in node.children.values())
    node.yellow_count = sum(child.yellow_count for child in node.children.values())
    node.green_count = sum(child.green_count for child in node.children.values())
    total_score = sum(child.completion * child.leaf_count for child in node.children.values())
    node.completion = total_score / node.leaf_count if node.leaf_count else 0.0
    if node.leaf_count and node.green_count == node.leaf_count:
        node.derived_status = "green"
    elif node.green_count or node.yellow_count:
        node.derived_status = "yellow"
    else:
        node.derived_status = "red"


def escape_toml_string(value: str) -> str:
    return json.dumps(value, ensure_ascii=True)


def seed_from_markdown(markdown_path: Path, output_path: Path) -> None:
    pattern = re.compile(
        r"^\|\s*`(?P<id>WOW(?:\.[A-Z0-9_]+)*)`\s*"
        r"\|\s*(?P<status>Red|Yellow|Green)\s*"
        r"\|\s*(?P<requirement>.*?)\s*"
        r"\|\s*(?P<proof>.*?)\s*\|$"
    )
    rows: list[dict[str, str]] = []
    for line in markdown_path.read_text(encoding="utf-8").splitlines():
        match = pattern.match(line)
        if not match:
            continue
        row = match.groupdict()
        if row["id"].startswith("WOW.EXAMPLE."):
            continue
        rows.append(row)

    if not rows:
        raise RuntimeError(f"no completion-tree rows found in {markdown_path}")

    lines = [
        "# WoW completion tree source of truth.",
        "# Edit leaf node statuses here; parent colors are derived by tools/render_wow_tree.py.",
        "",
    ]
    for row in rows:
        node_id = row["id"]
        required_for = ["CP2"] if node_id in CP2_LEAVES else []
        lines.extend(
            [
                "[[node]]",
                f'id = {escape_toml_string(node_id)}',
                f'label = {escape_toml_string(label_for_id(node_id))}',
                f'status = {escape_toml_string(row["status"].lower())}',
                f'requirement = {escape_toml_string(strip_markdown(row["requirement"]))}',
                f'proof = {escape_toml_string(strip_markdown(row["proof"]))}',
                "required_for = ["
                + ", ".join(escape_toml_string(tag) for tag in required_for)
                + "]",
                "",
            ]
        )
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text("\n".join(lines), encoding="utf-8", newline="\n")


def strip_markdown(value: str) -> str:
    value = re.sub(r"`([^`]+)`", r"\1", value)
    return html.unescape(value.strip())


def render_html(root: Node, output_path: Path, source_path: Path) -> None:
    output_path.parent.mkdir(parents=True, exist_ok=True)
    tree_json = json.dumps(root.to_dict(), ensure_ascii=True)
    html_text = f"""<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>WoW Completion Tree</title>
<style>
:root {{
  color-scheme: dark;
  --bg: #111312;
  --panel: #191d1b;
  --panel-2: #202521;
  --line: #343c35;
  --text: #edf2ee;
  --muted: #a9b6ad;
  --red: #d94d47;
  --yellow: #d7b84a;
  --green: #54b86a;
  --red-bg: rgba(217, 77, 71, .14);
  --yellow-bg: rgba(215, 184, 74, .14);
  --green-bg: rgba(84, 184, 106, .14);
  font-family: Inter, ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
}}
* {{ box-sizing: border-box; }}
body {{
  margin: 0;
  background: var(--bg);
  color: var(--text);
}}
header {{
  position: sticky;
  top: 0;
  z-index: 10;
  background: rgba(17, 19, 18, .94);
  border-bottom: 1px solid var(--line);
  backdrop-filter: blur(10px);
}}
.wrap {{ max-width: 1440px; margin: 0 auto; padding: 20px; }}
.topline {{ display: flex; align-items: end; justify-content: space-between; gap: 16px; }}
h1 {{ margin: 0 0 6px; font-size: 30px; letter-spacing: 0; }}
.subtle {{ color: var(--muted); font-size: 14px; }}
.controls {{ display: grid; grid-template-columns: minmax(220px, 1fr) auto auto auto; gap: 10px; margin-top: 16px; }}
input, select, button {{
  background: var(--panel);
  color: var(--text);
  border: 1px solid var(--line);
  border-radius: 6px;
  min-height: 40px;
  padding: 8px 10px;
  font: inherit;
}}
button {{ cursor: pointer; }}
button.active {{ border-color: var(--text); background: var(--panel-2); }}
main.wrap {{ display: grid; grid-template-columns: 360px minmax(0, 1fr); gap: 18px; align-items: start; }}
.panel {{
  background: var(--panel);
  border: 1px solid var(--line);
  border-radius: 8px;
  padding: 14px;
}}
.summary-grid {{ display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 10px; }}
.metric {{ background: var(--panel-2); border-radius: 6px; padding: 12px; }}
.metric strong {{ display: block; font-size: 24px; }}
.bar {{ height: 8px; background: #0d0f0e; border-radius: 999px; overflow: hidden; margin-top: 8px; }}
.bar > span {{ display: block; height: 100%; width: var(--pct); background: linear-gradient(90deg, var(--red), var(--yellow), var(--green)); }}
.system-list {{ display: grid; gap: 8px; margin-top: 12px; }}
.system-card {{
  border: 1px solid var(--line);
  border-radius: 6px;
  padding: 10px;
  background: var(--panel-2);
}}
.system-card h3 {{ margin: 0; font-size: 14px; }}
.system-card .counts {{ margin-top: 6px; color: var(--muted); font-size: 12px; }}
.view-toggle {{ display: inline-flex; gap: 6px; }}
.tree-stage {{
  background: var(--panel);
  border: 1px solid var(--line);
  border-radius: 8px;
  min-height: 720px;
  overflow: hidden;
  position: relative;
  touch-action: none;
  cursor: grab;
}}
.tree-stage.dragging {{ cursor: grabbing; }}
.tree-svg {{ display: block; width: 100%; height: 720px; }}
.tree-toolbar {{
  position: absolute;
  top: 12px;
  right: 12px;
  z-index: 2;
  display: flex;
  gap: 6px;
  align-items: center;
  background: rgba(17, 19, 18, .82);
  border: 1px solid var(--line);
  border-radius: 7px;
  padding: 6px;
}}
.tree-toolbar button {{ min-height: 32px; padding: 4px 9px; }}
.tree-help {{
  position: absolute;
  left: 14px;
  bottom: 12px;
  color: var(--muted);
  font-size: 12px;
  background: rgba(17, 19, 18, .82);
  border: 1px solid var(--line);
  border-radius: 999px;
  padding: 5px 10px;
  pointer-events: none;
}}
.tree-link {{
  fill: none;
  stroke: #485247;
  stroke-width: 1.4;
}}
.tree-node-card {{
  fill: var(--panel-2);
  stroke: var(--line);
  stroke-width: 1;
}}
.tree-node-card.red {{ stroke: rgba(217, 77, 71, .85); }}
.tree-node-card.yellow {{ stroke: rgba(215, 184, 74, .85); }}
.tree-node-card.green {{ stroke: rgba(84, 184, 106, .85); }}
.tree-node-dot.red {{ fill: var(--red); }}
.tree-node-dot.yellow {{ fill: var(--yellow); }}
.tree-node-dot.green {{ fill: var(--green); }}
.tree-node-label {{
  fill: var(--text);
  font-size: 12px;
  font-weight: 700;
}}
.tree-node-meta {{
  fill: var(--muted);
  font-size: 8px;
}}
.tree-node {{
  cursor: pointer;
}}
.tree-node.disabled {{
  cursor: grab;
}}
.tree-node-label {{
  font-size: 10px;
}}
.tree-node-toggle {{
  fill: var(--text);
  font-size: 11px;
  font-weight: 800;
  pointer-events: none;
}}
.tree-level-line {{
  stroke: rgba(169, 182, 173, .14);
  stroke-width: 1;
}}
.tree-level-label {{
  fill: var(--muted);
  font-size: 11px;
  letter-spacing: 0;
}}
.tree-empty {{ padding: 24px; color: var(--muted); }}
.outline-tree {{ display: grid; gap: 8px; }}
details.node {{
  border: 1px solid var(--line);
  border-radius: 8px;
  background: var(--panel);
  overflow: hidden;
}}
details.node[open] > summary {{ border-bottom: 1px solid var(--line); }}
summary {{
  list-style: none;
  cursor: pointer;
  padding: 10px 12px;
}}
summary::-webkit-details-marker {{ display: none; }}
.row {{ display: grid; grid-template-columns: auto minmax(0, 1fr) auto; gap: 10px; align-items: center; }}
.chevron {{ color: var(--muted); font-size: 12px; width: 14px; }}
details[open] > summary .chevron {{ transform: rotate(90deg); }}
.node-title {{ min-width: 0; }}
.node-title b {{ display: block; font-size: 14px; overflow-wrap: anywhere; }}
.node-title code {{ color: var(--muted); font-size: 11px; }}
.pill {{
  border-radius: 999px;
  padding: 4px 8px;
  font-size: 12px;
  font-weight: 700;
  min-width: 62px;
  text-align: center;
}}
.pill.red {{ color: #ffd9d7; background: var(--red-bg); border: 1px solid rgba(217,77,71,.45); }}
.pill.yellow {{ color: #fff1bf; background: var(--yellow-bg); border: 1px solid rgba(215,184,74,.45); }}
.pill.green {{ color: #d9ffe1; background: var(--green-bg); border: 1px solid rgba(84,184,106,.45); }}
.node-body {{ padding: 10px 12px 12px 32px; }}
.requirement {{ margin: 0 0 8px; color: var(--text); }}
.proof {{ margin: 0 0 10px; color: var(--muted); font-size: 13px; }}
.tags {{ display: flex; flex-wrap: wrap; gap: 6px; margin-bottom: 8px; }}
.tag {{ border: 1px solid var(--line); border-radius: 999px; padding: 2px 7px; color: var(--muted); font-size: 12px; }}
.children {{ display: grid; gap: 8px; margin-top: 8px; }}
.hidden {{ display: none !important; }}
@media (max-width: 980px) {{
  main.wrap {{ grid-template-columns: 1fr; }}
  .controls {{ grid-template-columns: 1fr; }}
  .topline {{ display: block; }}
}}
</style>
</head>
<body>
<header>
  <div class="wrap">
    <div class="topline">
      <div>
        <h1>WoW Completion Tree</h1>
        <div class="subtle">Source: {html.escape(source_path.as_posix())}. Parent status is computed from leaf requirements.</div>
      </div>
      <div class="subtle" id="updated"></div>
    </div>
    <div class="controls">
      <input id="search" type="search" placeholder="Search quests, Heroic Strike, loot, relog...">
      <select id="tagFilter">
        <option value="all">All milestone tags</option>
        <option value="CP2">CP2 Northshire</option>
      </select>
      <div>
        <button data-status="all" class="active">All</button>
        <button data-status="red">Red</button>
        <button data-status="yellow">Yellow</button>
        <button data-status="green">Green</button>
      </div>
      <div class="view-toggle">
        <button data-view="tree" class="active">Tree</button>
        <button data-view="outline">Outline</button>
      </div>
    </div>
  </div>
</header>
<main class="wrap">
  <aside class="panel">
    <div class="summary-grid" id="summary"></div>
    <div class="system-list" id="systems"></div>
  </aside>
  <section class="tree-stage" id="tree"></section>
</main>
<script>
const tree = {tree_json};
let statusFilter = "all";
let viewMode = "tree";
let treeZoom = 0.86;
let treePan = {{ x: 40, y: 34 }};
let treePointer = null;
let treeDragged = false;
let treeNeedsReset = true;
let currentTreeFocusId = "WOW";
let currentTreeFocus = {{ x: 0, y: 0, cardWidth: 132, y: 54 }};
const expandedNodeIds = new Set();
const search = document.getElementById("search");
const tagFilter = document.getElementById("tagFilter");
document.getElementById("updated").textContent = "Generated " + new Date().toLocaleString();

function pct(node) {{ return Math.round(node.completion * 100); }}
function counts(node) {{ return `${{node.greenCount}} green, ${{node.yellowCount}} yellow, ${{node.redCount}} red`; }}
function hasActiveFilter() {{
  return search.value.trim() || tagFilter.value !== "all" || statusFilter !== "all";
}}
function selfMatches(node) {{
  const query = search.value.trim().toLowerCase();
  const tag = tagFilter.value;
  const text = [node.id, node.label, node.requirement, node.proof, ...(node.requiredFor || [])].join(" ").toLowerCase();
  const queryOk = !query || text.includes(query);
  const statusOk = statusFilter === "all" || node.status === statusFilter;
  const tagOk = tag === "all" || (node.requiredFor || []).includes(tag);
  return queryOk && statusOk && tagOk;
}}
function filteredTree(node) {{
  const children = node.children.map(filteredTree).filter(Boolean);
  if (selfMatches(node) || children.length) {{
    return {{ ...node, actualChildCount: node.children.length, children }};
  }}
  return null;
}}
function nodeHtml(node, depth = 0) {{
  const visible = filteredTree(node);
  if (!visible) return "";
  node = visible;
  const open = depth < 2 || search.value || tagFilter.value !== "all" || statusFilter !== "all";
  const childHtml = node.children.map(child => nodeHtml(child, depth + 1)).filter(Boolean).join("");
  const tags = (node.requiredFor || []).map(tag => `<span class="tag">${{escapeHtml(tag)}}</span>`).join("");
  return `<details class="node" ${{open ? "open" : ""}}>
    <summary>
      <div class="row">
        <span class="chevron">${{node.children.length ? ">" : ""}}</span>
        <span class="node-title"><b>${{escapeHtml(node.label)}}</b><code>${{escapeHtml(node.id)}}</code></span>
        <span class="pill ${{node.status}}">${{capitalize(node.status)}} ${{pct(node)}}%</span>
      </div>
      <div class="bar" style="--pct:${{pct(node)}}%"><span></span></div>
    </summary>
    <div class="node-body">
      ${{tags ? `<div class="tags">${{tags}}</div>` : ""}}
      ${{node.requirement ? `<p class="requirement">${{escapeHtml(node.requirement)}}</p>` : ""}}
      ${{node.proof ? `<p class="proof">${{escapeHtml(node.proof)}}</p>` : ""}}
      <div class="subtle">${{node.leafCount}} leaves: ${{counts(node)}}</div>
      ${{childHtml ? `<div class="children">${{childHtml}}</div>` : ""}}
    </div>
  </details>`;
}}
function visibleTreeData() {{
  if (hasActiveFilter()) return filteredTree(tree);
  function compactNode(node) {{
    const isExpanded = expandedNodeIds.has(node.id);
    return {{
      ...node,
      actualChildCount: node.children.length,
      children: isExpanded ? node.children.map(compactNode) : [],
    }};
  }}
  return compactNode(tree);
}}
function findNodePath(node, targetId, path = []) {{
  const nextPath = [...path, node.id];
  if (node.id === targetId) return nextPath;
  for (const child of node.children) {{
    const result = findNodePath(child, targetId, nextPath);
    if (result) return result;
  }}
  return null;
}}
function setExpandedPathTo(nodeId, collapseIfAlreadyExpanded = false) {{
  const path = findNodePath(tree, nodeId);
  if (!path) return;
  const shouldCollapse = collapseIfAlreadyExpanded && expandedNodeIds.has(nodeId);
  expandedNodeIds.clear();
  const nextPath = shouldCollapse ? path.slice(0, -1) : path;
  nextPath.forEach(id => expandedNodeIds.add(id));
}}
function collectTreeLayout(root) {{
  const nodes = [];
  const links = [];
  function build(node, depth, parent, centerX) {{
    const layoutNode = {{
      ...node,
      depth,
      parent,
      x: centerX - (depth === 0 ? 132 : 138) / 2,
      y: 54 + depth * 104,
      cardWidth: depth === 0 ? 132 : 138,
      cardHeight: 30,
      children: [],
    }};
    nodes.push(layoutNode);
    if (parent) {{
      links.push({{ source: parent, target: layoutNode }});
    }}
    const childGap = 152;
    const firstChildCenter = centerX - ((node.children.length - 1) * childGap) / 2;
    layoutNode.children = node.children.map((child, index) =>
      build(child, depth + 1, layoutNode, firstChildCenter + index * childGap)
    );
    return layoutNode;
  }}
  build(root, 0, null, 700);
  return {{ nodes, links }};
}}
function renderSvgTree() {{
  const root = visibleTreeData();
  if (!root) {{
    return '<div class="tree-empty">No nodes match the current filters.</div>';
  }}
  const layout = collectTreeLayout(root);
  currentTreeFocus = layout.nodes.find(node => node.id === currentTreeFocusId) || layout.nodes[0];
  const maxDepth = Math.max(...layout.nodes.map(node => node.depth));
  const minX = Math.min(...layout.nodes.map(node => node.x));
  const maxX = Math.max(...layout.nodes.map(node => node.x + node.cardWidth));
  const bandWidth = Math.max(1400, maxX - minX + 160);
  const bandStart = Math.min(-40, minX - 80);
  const levelBands = Array.from({{ length: maxDepth + 1 }}, (_, depth) => {{
    const y = 54 + depth * 104;
    return `<line class="tree-level-line" x1="${{bandStart}}" y1="${{y - 15}}" x2="${{bandStart + bandWidth}}" y2="${{y - 15}}"></line>
      <text class="tree-level-label" x="${{bandStart + 8}}" y="${{y - 22}}">Level ${{depth}}</text>`;
  }}).join("");
  const links = layout.links.map(link => {{
    const x1 = link.source.x + link.source.cardWidth / 2;
    const y1 = link.source.y + link.source.cardHeight;
    const x2 = link.target.x + link.target.cardWidth / 2;
    const y2 = link.target.y;
    const mid = Math.max(30, (y2 - y1) / 2);
    return `<path class="tree-link" d="M ${{x1}} ${{y1}} C ${{x1}} ${{y1 + mid}}, ${{x2}} ${{y2 - mid}}, ${{x2}} ${{y2}}"></path>`;
  }}).join("");
  const nodes = layout.nodes.map(node => {{
    const label = truncate(node.label, node.depth === 0 ? 17 : 18);
    const meta = `${{capitalize(node.status)}} ${{pct(node)}}% - ${{node.leafCount}}`;
    const tags = (node.requiredFor || []).join(", ");
    const title = escapeHtml(`${{node.id}}\\n${{node.requirement || node.label}}\\n${{counts(node)}}${{tags ? "\\n" + tags : ""}}`);
    const canExpand = (node.actualChildCount || 0) > 0 && !hasActiveFilter();
    const toggle = canExpand ? (expandedNodeIds.has(node.id) ? "-" : "+") : "";
    const nodeClass = canExpand ? "tree-node" : "tree-node disabled";
    const clickAttr = canExpand ? ` onclick="event.stopPropagation(); toggleTreeNode('${{node.id}}')"` : "";
    return `<g class="${{nodeClass}}" data-node-id="${{escapeHtml(node.id)}}" data-can-expand="${{canExpand ? "true" : "false"}}" transform="translate(${{node.x}}, ${{node.y}})"${{clickAttr}}>
      <title>${{title}}</title>
      <rect class="tree-node-card ${{node.status}}" width="${{node.cardWidth}}" height="${{node.cardHeight}}" rx="7"></rect>
      <circle class="tree-node-dot ${{node.status}}" cx="12" cy="12" r="4.5"></circle>
      <text class="tree-node-label" x="22" y="13">${{escapeHtml(label)}}</text>
      <text class="tree-node-meta" x="22" y="24">${{escapeHtml(meta)}}</text>
      ${{toggle ? `<text class="tree-node-toggle" x="${{node.cardWidth - 14}}" y="19">${{toggle}}</text>` : ""}}
    </g>`;
  }}).join("");
  return `<div class="tree-toolbar">
      <button data-tree-action="zoom-out" title="Zoom out">-</button>
      <button data-tree-action="reset" title="Reset view">Reset</button>
      <button data-tree-action="zoom-in" title="Zoom in">+</button>
    </div>
    <svg class="tree-svg" viewBox="0 0 1400 900" role="img" aria-label="WoW completion tree node-link diagram">
      <g id="treeViewport">${{levelBands}}${{links}}${{nodes}}</g>
    </svg>
    <div class="tree-help">Each horizontal band is one tree level. Click nodes to expand. Drag to pan. Mouse wheel or +/- to zoom.</div>`;
}}
function truncate(value, limit) {{
  value = String(value);
  return value.length > limit ? value.slice(0, limit - 3) + "..." : value;
}}
function clamp(value, min, max) {{
  return Math.max(min, Math.min(max, value));
}}
function svgUnitScale(svg) {{
  const rect = svg.getBoundingClientRect();
  const viewBox = svg.viewBox.baseVal;
  return {{
    x: viewBox.width / Math.max(1, rect.width),
    y: viewBox.height / Math.max(1, rect.height),
  }};
}}
function updateTreeTransform() {{
  const viewport = document.getElementById("treeViewport");
  if (!viewport) return;
  viewport.setAttribute("transform", `translate(${{treePan.x}} ${{treePan.y}}) scale(${{treeZoom}})`);
}}
function resetTreeView() {{
  treeZoom = 0.86;
  treePan = {{
    x: 700 - (currentTreeFocus.x + currentTreeFocus.cardWidth / 2) * treeZoom,
    y: 188 - currentTreeFocus.y * treeZoom,
  }};
  treeNeedsReset = false;
  updateTreeTransform();
}}
function toggleTreeNode(nodeId) {{
  if (treeDragged) {{
    treeDragged = false;
    return;
  }}
  if (hasActiveFilter()) return;
  setExpandedPathTo(nodeId, true);
  currentTreeFocusId = nodeId;
  treeNeedsReset = true;
  renderTree();
}}
function zoomTree(factor, centerClientX = null, centerClientY = null) {{
  const svg = document.querySelector(".tree-svg");
  if (!svg) return;
  const rect = svg.getBoundingClientRect();
  const scale = svgUnitScale(svg);
  const centerX = centerClientX === null ? rect.left + rect.width / 2 : centerClientX;
  const centerY = centerClientY === null ? rect.top + rect.height / 2 : centerClientY;
  const svgX = (centerX - rect.left) * scale.x;
  const svgY = (centerY - rect.top) * scale.y;
  const oldZoom = treeZoom;
  const newZoom = clamp(treeZoom * factor, 0.18, 2.4);
  const contentX = (svgX - treePan.x) / oldZoom;
  const contentY = (svgY - treePan.y) / oldZoom;
  treeZoom = newZoom;
  treePan.x = svgX - contentX * newZoom;
  treePan.y = svgY - contentY * newZoom;
  updateTreeTransform();
}}
function attachTreePanZoom() {{
  const stage = document.getElementById("tree");
  const svg = document.querySelector(".tree-svg");
  if (!stage || !svg) return;
  if (treeNeedsReset) resetTreeView();
  updateTreeTransform();
  stage.querySelectorAll("button[data-tree-action]").forEach(button => {{
    button.addEventListener("click", event => {{
      event.stopPropagation();
      const action = button.dataset.treeAction;
      if (action === "zoom-in") zoomTree(1.22);
      if (action === "zoom-out") zoomTree(1 / 1.22);
      if (action === "reset") resetTreeView();
    }});
  }});
  svg.addEventListener("wheel", event => {{
    event.preventDefault();
    zoomTree(event.deltaY < 0 ? 1.12 : 1 / 1.12, event.clientX, event.clientY);
  }}, {{ passive: false }});
  svg.addEventListener("click", event => {{
    const node = event.target.closest("g.tree-node[data-can-expand='true']");
    if (!node || treeDragged) {{
      treeDragged = false;
      return;
    }}
    event.stopPropagation();
  }});
  svg.addEventListener("pointerdown", event => {{
    if (event.button !== 0) return;
    if (event.target.closest("g.tree-node[data-can-expand='true']")) return;
    stage.classList.add("dragging");
    svg.setPointerCapture(event.pointerId);
    treePointer = {{ id: event.pointerId, x: event.clientX, y: event.clientY }};
    treeDragged = false;
  }});
  svg.addEventListener("pointermove", event => {{
    if (!treePointer || treePointer.id !== event.pointerId) return;
    const scale = svgUnitScale(svg);
    const dx = event.clientX - treePointer.x;
    const dy = event.clientY - treePointer.y;
    if (Math.abs(dx) + Math.abs(dy) > 3) treeDragged = true;
    treePan.x += dx * scale.x;
    treePan.y += dy * scale.y;
    treePointer = {{ id: event.pointerId, x: event.clientX, y: event.clientY }};
    updateTreeTransform();
  }});
  function stopDrag(event) {{
    if (treePointer && treePointer.id === event.pointerId) {{
      treePointer = null;
      stage.classList.remove("dragging");
    }}
  }}
  svg.addEventListener("pointerup", stopDrag);
  svg.addEventListener("pointercancel", stopDrag);
}}
function renderSummary() {{
  document.getElementById("summary").innerHTML = `
    <div class="metric"><span class="subtle">Completion</span><strong>${{pct(tree)}}%</strong><div class="bar" style="--pct:${{pct(tree)}}%"><span></span></div></div>
    <div class="metric"><span class="subtle">Leaves</span><strong>${{tree.leafCount}}</strong><div class="subtle">${{counts(tree)}}</div></div>
    <div class="metric"><span class="subtle">Green</span><strong>${{tree.greenCount}}</strong></div>
    <div class="metric"><span class="subtle">Open Risk</span><strong>${{tree.redCount + tree.yellowCount}}</strong></div>`;
  const systems = tree.children.map(system => `
    <button class="system-card" type="button" onclick="focusSystem('${{system.id}}')">
      <h3>${{escapeHtml(system.label)}}</h3>
      <div class="bar" style="--pct:${{pct(system)}}%"><span></span></div>
      <div class="counts">${{pct(system)}}% - ${{counts(system)}}</div>
    </button>`).join("");
  document.getElementById("systems").innerHTML = systems;
}}
function renderTree() {{
  document.getElementById("tree").innerHTML = viewMode === "tree" ? renderSvgTree() : `<div class="outline-tree">${{nodeHtml(tree)}}</div>`;
  if (viewMode === "tree") attachTreePanZoom();
}}
function focusSystem(id) {{
  search.value = "";
  viewMode = "tree";
  setExpandedPathTo(id, false);
  currentTreeFocusId = id;
  treeNeedsReset = true;
  document.querySelectorAll("button[data-view]").forEach(item => item.classList.toggle("active", item.dataset.view === viewMode));
  renderTree();
}}
function escapeHtml(value) {{
  return String(value).replace(/[&<>"']/g, char => ({{"&":"&amp;","<":"&lt;",">":"&gt;","\\"":"&quot;","'":"&#39;"}}[char]));
}}
function capitalize(value) {{ return value.charAt(0).toUpperCase() + value.slice(1); }}
document.querySelectorAll("button[data-status]").forEach(button => {{
  button.addEventListener("click", () => {{
    document.querySelectorAll("button[data-status]").forEach(item => item.classList.remove("active"));
    button.classList.add("active");
    statusFilter = button.dataset.status;
    currentTreeFocusId = "WOW";
    treeNeedsReset = true;
    renderTree();
  }});
}});
document.querySelectorAll("button[data-view]").forEach(button => {{
  button.addEventListener("click", () => {{
    document.querySelectorAll("button[data-view]").forEach(item => item.classList.remove("active"));
    button.classList.add("active");
    viewMode = button.dataset.view;
    currentTreeFocusId = "WOW";
    treeNeedsReset = true;
    renderTree();
  }});
}});
search.addEventListener("input", () => {{
  currentTreeFocusId = "WOW";
  treeNeedsReset = true;
  renderTree();
}});
tagFilter.addEventListener("change", () => {{
  currentTreeFocusId = "WOW";
  treeNeedsReset = true;
  renderTree();
}});
renderSummary();
renderTree();
</script>
</body>
</html>
"""
    output_path.write_text(html_text, encoding="utf-8", newline="\n")


def main(argv: list[str]) -> int:
    root_dir = repo_root()
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--source",
        type=Path,
        default=root_dir / "docs" / "wow_completion_tree.toml",
        help="TOML source file",
    )
    parser.add_argument(
        "--output",
        type=Path,
        default=root_dir / "docs" / "generated" / "wow_completion_tree.html",
        help="HTML output file",
    )
    parser.add_argument(
        "--seed-from-markdown",
        action="store_true",
        help="Create the TOML source from docs/wow_completion_tree.md",
    )
    args = parser.parse_args(argv)
    source_path = args.source if args.source.is_absolute() else root_dir / args.source
    output_path = args.output if args.output.is_absolute() else root_dir / args.output

    if args.seed_from_markdown:
        seed_from_markdown(root_dir / "docs" / "wow_completion_tree.md", source_path)

    rows = load_toml(source_path)
    root = build_tree(rows)
    render_html(root, output_path, source_path.resolve().relative_to(root_dir))
    print(f"rendered {output_path}")
    print(
        f"{STATUS_LABEL[root.derived_status.lower()]} {root.completion:.0%}: "
        f"{root.green_count} green, {root.yellow_count} yellow, "
        f"{root.red_count} red across {root.leaf_count} leaves"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
