import { useEffect, useRef } from "react";
import {
  IdleAnimation,
  NameTagObject,
  RunningAnimation,
  SkinViewer,
  WalkingAnimation,
  type PlayerAnimation,
} from "skinview3d";

import type { SkinModel } from "../../lib/ipc/bindings/SkinModel";

export type Pose = "idle" | "walk" | "run";
export type BackItem = "cape" | "elytra";

function makeAnimation(pose: Pose): PlayerAnimation {
  switch (pose) {
    case "walk":
      return new WalkingAnimation();
    case "run": {
      const a = new RunningAnimation();
      a.speed = 0.7;
      return a;
    }
    default:
      return new IdleAnimation();
  }
}

/** Rotatable 3D player (three.js via skinview3d); drag to turn, wheel to zoom. */
export function SkinViewer3D({
  skin,
  model,
  cape,
  back,
  pose,
  name,
  className = "",
}: {
  skin: string | null;
  model: SkinModel;
  cape: string | null;
  back: BackItem;
  pose: Pose;
  name: string | null;
  className?: string;
}) {
  const host = useRef<HTMLDivElement>(null);
  const canvas = useRef<HTMLCanvasElement>(null);
  const viewer = useRef<SkinViewer | null>(null);

  // One WebGL context for the component's lifetime.
  useEffect(() => {
    const el = host.current;
    if (!el || !canvas.current) return;
    const v = new SkinViewer({
      canvas: canvas.current,
      width: el.clientWidth || 300,
      height: el.clientHeight || 400,
      zoom: 0.7,
      fov: 50,
    });
    v.controls.enablePan = false;
    v.playerWrapper.rotation.y = -0.45;
    viewer.current = v;

    const ro = new ResizeObserver(() => {
      if (el.clientWidth > 0 && el.clientHeight > 0) v.setSize(el.clientWidth, el.clientHeight);
    });
    ro.observe(el);
    // Stop rendering while the window is hidden.
    const onVisibility = () => (v.renderPaused = document.hidden);
    document.addEventListener("visibilitychange", onVisibility);
    return () => {
      ro.disconnect();
      document.removeEventListener("visibilitychange", onVisibility);
      v.dispose();
      viewer.current = null;
    };
  }, []);

  useEffect(() => {
    const v = viewer.current;
    if (!v) return;
    if (skin) void v.loadSkin(skin, { model: model === "slim" ? "slim" : "default" });
    else v.loadSkin(null);
  }, [skin, model]);

  useEffect(() => {
    const v = viewer.current;
    if (!v) return;
    if (cape) void v.loadCape(cape, { backEquipment: back });
    else v.loadCape(null);
  }, [cape, back]);

  useEffect(() => {
    const v = viewer.current;
    if (v) v.animation = makeAnimation(pose);
  }, [pose]);

  useEffect(() => {
    const v = viewer.current;
    if (v)
      v.nameTag = name ? new NameTagObject(name, { font: "600 48px Rajdhani, sans-serif" }) : null;
  }, [name]);

  return (
    <div ref={host} className={`relative ${className}`}>
      <canvas ref={canvas} className="absolute inset-0 cursor-grab active:cursor-grabbing" />
    </div>
  );
}
