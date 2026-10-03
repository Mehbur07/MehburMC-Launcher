import {
  Box,
  Castle,
  Flame,
  Gem,
  Pickaxe,
  Rocket,
  Skull,
  Sprout,
  Sword,
  TreePine,
  type LucideIcon,
} from "lucide-react";

/** Preset instance icons: key → glyph + tint. Keys are stored in instance.json. */
export const INSTANCE_ICONS: Record<string, { icon: LucideIcon; tint: string }> = {
  grass: { icon: Sprout, tint: "#39ff88" },
  pickaxe: { icon: Pickaxe, tint: "#00f0ff" },
  sword: { icon: Sword, tint: "#7df9ff" },
  cube: { icon: Box, tint: "#00b8d4" },
  flame: { icon: Flame, tint: "#ff8a3d" },
  gem: { icon: Gem, tint: "#b77cff" },
  rocket: { icon: Rocket, tint: "#ff3df0" },
  castle: { icon: Castle, tint: "#ffb800" },
  tree: { icon: TreePine, tint: "#1fc266" },
  skull: { icon: Skull, tint: "#e6f1f5" },
};

export function InstanceIcon({ icon, size = 48 }: { icon: string; size?: number }) {
  const { icon: Glyph, tint } = INSTANCE_ICONS[icon] ?? INSTANCE_ICONS.grass!;
  return (
    <div
      className="grid shrink-0 place-items-center rounded-lg border"
      style={{
        width: size,
        height: size,
        borderColor: `${tint}55`,
        background: `radial-gradient(circle at 30% 25%, ${tint}33, transparent 70%), var(--mc-surface-2)`,
        boxShadow: `inset 0 0 12px ${tint}22`,
      }}
    >
      <Glyph
        size={Math.round(size * 0.5)}
        style={{ color: tint, filter: `drop-shadow(0 0 6px ${tint}88)` }}
      />
    </div>
  );
}
