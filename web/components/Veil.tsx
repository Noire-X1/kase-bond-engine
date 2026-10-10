"use client";
import { useEffect, useRef, useState } from "react";

const VERT = `attribute vec2 p; void main(){ gl_Position = vec4(p, 0.0, 1.0); }`;
const FRAG = `
#ifdef GL_FRAGMENT_PRECISION_HIGH
precision highp float;
#else
precision mediump float;
#endif
uniform vec2 r; uniform float t; uniform vec2 m; uniform float mp;
float field(vec2 q){ return 0.5 + 0.5 * sin(q.x * 3.0 + t) * sin(q.y * 2.4 - t * 0.7); }
void main(){
  float s = min(r.x, r.y);
  vec2 p = (gl_FragCoord.xy - 0.5 * r) / s;
  vec2 c = (m - 0.5 * r) / s;
  vec2 d = p - c;
  float dist = length(d);
  float mask = mp * (1.0 - smoothstep(0.0, 0.5, dist));
  float a = mask * 2.2;
  p = c + mat2(cos(a), -sin(a), sin(a), cos(a)) * d;
  p += (d / max(dist, 0.0001)) * sin(dist * 18.0 - t * 5.0) * mask * 0.03;
  float f = field(p * 1.1);
  vec2 g = fract(p * 30.0) - 0.5;
  float rad = 0.10 + f * 0.20 + mask * 0.12;
  float dotm = 1.0 - smoothstep(rad - 0.07, rad, length(g));
  vec3 bg = vec3(0.02, 0.07, 0.055);
  vec3 green = vec3(0.05, 0.22, 0.16);
  vec3 sky = vec3(0.42, 0.77, 0.91);
  vec3 gold = vec3(0.79, 0.64, 0.30);
  vec3 col = f < 0.5 ? mix(green, sky, f * 2.0) : mix(sky, gold, (f - 0.5) * 2.0);
  col = mix(col, gold, mask * 0.6);
  gl_FragColor = vec4(mix(bg, col * 0.55, dotm), 1.0);
}`;

export default function Veil() {
  const ref = useRef<HTMLCanvasElement>(null);
  const [on, setOn] = useState(true);

  useEffect(() => {
    if (!on) return;
    const canvas = ref.current;
    if (!canvas) return;
    const gl = canvas.getContext("webgl", { antialias: false });
    if (!gl) return;
    const reduce = window.matchMedia("(prefers-reduced-motion: reduce)").matches;

    const mk = (type: number, src: string) => {
      const s = gl.createShader(type);
      if (!s) return null;
      gl.shaderSource(s, src);
      gl.compileShader(s);
      return gl.getShaderParameter(s, gl.COMPILE_STATUS) ? s : null;
    };
    const vs = mk(gl.VERTEX_SHADER, VERT);
    const fs = mk(gl.FRAGMENT_SHADER, FRAG);
    const prog = gl.createProgram();
    if (!vs || !fs || !prog) return;
    gl.attachShader(prog, vs);
    gl.attachShader(prog, fs);
    gl.linkProgram(prog);
    if (!gl.getProgramParameter(prog, gl.LINK_STATUS)) return;
    gl.useProgram(prog);

    const buf = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, buf);
    gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1, -1, 3, -1, -1, 3]), gl.STATIC_DRAW);
    const loc = gl.getAttribLocation(prog, "p");
    gl.enableVertexAttribArray(loc);
    gl.vertexAttribPointer(loc, 2, gl.FLOAT, false, 0, 0);
    const uR = gl.getUniformLocation(prog, "r");
    const uT = gl.getUniformLocation(prog, "t");
    const uM = gl.getUniformLocation(prog, "m");
    const uP = gl.getUniformLocation(prog, "mp");

    let raf = 0;
    let tx = 0, ty = 0, tp = 0, mx = 0, my = 0, mp = 0;
    let last = performance.now();
    const t0 = last;

    const resize = () => {
      const w = Math.max(1, window.innerWidth);
      const h = Math.max(1, window.innerHeight);
      const k = Math.min(1, Math.sqrt(1_000_000 / (w * h)));
      canvas.width = Math.round(w * k);
      canvas.height = Math.round(h * k);
      gl.viewport(0, 0, canvas.width, canvas.height);
    };
    const draw = (t: number) => {
      gl.uniform2f(uR, canvas.width, canvas.height);
      gl.uniform1f(uT, t);
      gl.uniform2f(uM, mx, my);
      gl.uniform1f(uP, mp);
      gl.drawArrays(gl.TRIANGLES, 0, 3);
    };
    const render = (now: number) => {
      raf = requestAnimationFrame(render);
      if (document.hidden) { last = now; return; }
      const dt = Math.min((now - last) / 1000, 0.1);
      last = now;
      const f = 1 - Math.exp(-10 * dt);
      mx += (tx - mx) * f;
      my += (ty - my) * f;
      mp += (tp - mp) * f;
      draw(((now - t0) / 1000) * 0.5);
    };
    const onMove = (e: PointerEvent) => {
      const kx = canvas.width / window.innerWidth;
      const ky = canvas.height / window.innerHeight;
      tx = e.clientX * kx;
      ty = (window.innerHeight - e.clientY) * ky;
      if (mp < 0.02) { mx = tx; my = ty; }
      tp = 1;
    };
    const onLeave = () => { tp = 0; };
    const onResize = () => { resize(); if (reduce) draw(2); };

    resize();
    window.addEventListener("resize", onResize);
    if (reduce) {
      draw(2);
    } else {
      window.addEventListener("pointermove", onMove, { passive: true });
      document.documentElement.addEventListener("pointerleave", onLeave);
      raf = requestAnimationFrame(render);
    }
    return () => {
      cancelAnimationFrame(raf);
      window.removeEventListener("resize", onResize);
      window.removeEventListener("pointermove", onMove);
      document.documentElement.removeEventListener("pointerleave", onLeave);
      gl.deleteProgram(prog);
      gl.deleteBuffer(buf);
    };
  }, [on]);

  return (
    <>
      {on && <canvas ref={ref} className="tg-veil" aria-hidden="true" />}
      <button type="button" className="tg-veil-toggle" onClick={() => setOn((v) => !v)} aria-pressed={on}>
        Texture: {on ? "on" : "off"}
      </button>
    </>
  );
}
