"use client";
import { useEffect } from "react";

export default function Fx() {
  useEffect(() => {
    const root = document.documentElement;
    const reduce = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    root.classList.add("js-fx");

    const bar = document.getElementById("tg-progress");
    let ticking = false;
    const onScroll = () => {
      if (ticking) return;
      ticking = true;
      requestAnimationFrame(() => {
        const h = root.scrollHeight - window.innerHeight;
        const p = h > 0 ? window.scrollY / h : 0;
        if (bar) bar.style.transform = `scaleX(${p})`;
        if (!reduce) root.style.setProperty("--sy", String(window.scrollY));
        ticking = false;
      });
    };
    onScroll();
    window.addEventListener("scroll", onScroll, { passive: true });

    const onMove = (e: PointerEvent) => {
      root.style.setProperty("--mx", e.clientX + "px");
      root.style.setProperty("--my", e.clientY + "px");
      const t = e.target as HTMLElement | null;
      const card = t && t.closest ? (t.closest(".tg-card") as HTMLElement | null) : null;
      if (card) {
        const r = card.getBoundingClientRect();
        card.style.setProperty("--cx", e.clientX - r.left + "px");
        card.style.setProperty("--cy", e.clientY - r.top + "px");
      }
    };
    if (!reduce) window.addEventListener("pointermove", onMove, { passive: true });

    const count = (el: HTMLElement) => {
      const end = Number(el.dataset.count);
      if (reduce || !isFinite(end)) {
        el.textContent = String(end);
        return;
      }
      const t0 = performance.now();
      const dur = 1400;
      const tick = (t: number) => {
        const k = Math.min(1, (t - t0) / dur);
        el.textContent = String(Math.round(end * (1 - Math.pow(1 - k, 3))));
        if (k < 1) requestAnimationFrame(tick);
      };
      el.textContent = "0";
      requestAnimationFrame(tick);
    };

    const io = new IntersectionObserver(
      (entries) => {
        entries.forEach((en) => {
          if (!en.isIntersecting) return;
          const el = en.target as HTMLElement;
          el.classList.add("is-in");
          if (el.dataset.count !== undefined) count(el);
          io.unobserve(el);
        });
      },
      { threshold: 0.15 }
    );
    document
      .querySelectorAll<HTMLElement>("[data-reveal],[data-count]")
      .forEach((el) => io.observe(el));

    return () => {
      window.removeEventListener("scroll", onScroll);
      window.removeEventListener("pointermove", onMove);
      io.disconnect();
    };
  }, []);

  return <div id="tg-progress" className="tg-progress" aria-hidden="true" />;
}
