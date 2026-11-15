// Android'in geri tuşu (K103): WebView geri gidebiliyorsa geri gider, yoksa
// uygulamayı kapatır. Sayfa değişimlerini tarayıcı geçmişine yazınca geri tuşu
// önceki sayfaya döner; ilk sayfada uygulamadan çıkar.
import { useEffect, useRef } from "react";

import { IS_MOBILE } from "../lib/platform";
import { useApp, type View } from "../stores/app";

type Entry = { view?: View; drawer?: boolean; layer?: number } | null;

export function useMobileHistory(enabled: boolean) {
  useEffect(() => {
    if (!enabled) return;
    let fromPop = false;
    window.history.replaceState({ view: useApp.getState().view }, "");
    const unsubscribe = useApp.subscribe((s, prev) => {
      if (s.view === prev.view) return;
      if (fromPop) {
        fromPop = false;
        return;
      }
      window.history.pushState({ view: s.view }, "");
    });
    const onPop = (e: PopStateEvent) => {
      const view = (e.state as Entry)?.view;
      if (view && view !== useApp.getState().view) {
        fromPop = true;
        useApp.getState().setView(view);
      }
    };
    window.addEventListener("popstate", onPop);
    return () => {
      unsubscribe();
      window.removeEventListener("popstate", onPop);
    };
  }, [enabled]);
}

// Açık katman sayısı. Her katman kendi derinliğini geçmiş kaydına yazar; geri
// tuşu yalnız en üstteki katmanı kapatır (pencere sohbetin üstündeyse önce pencere).
let depth = 0;

/**
 * Telefonda açık bir katman (pencere, tam ekran sohbet): açılınca geçmişe bir
 * kayıt eklenir, geri tuşu önce katmanı kapatır. Katman başka yoldan kapanırsa
 * (Kapat düğmesi) eklenen kayıt geri alınır. Kayıt o anki sayfayı da taşır;
 * katman açıkken başka sayfaya geçilip geri gelinirse aynı sayfaya dönülür.
 */
export function useBackLayer(open: boolean, close: () => void, enabled = IS_MOBILE) {
  const closeRef = useRef(close);
  useEffect(() => {
    closeRef.current = close;
  });
  useEffect(() => {
    if (!enabled || !open) return;
    const mine = ++depth;
    // Aynı derinlikteki kayıt zaten en üstteyse (katman hemen yeniden açıldı)
    // yeni kayıt eklenmez.
    if ((window.history.state as Entry)?.layer !== mine) {
      window.history.pushState({ view: useApp.getState().view, layer: mine }, "");
    }
    const onPop = (e: PopStateEvent) => {
      if (((e.state as Entry)?.layer ?? 0) < mine) closeRef.current();
    };
    window.addEventListener("popstate", onPop);
    return () => {
      depth--;
      window.removeEventListener("popstate", onPop);
      // Kayıt bir an sonra geri alınır: katman o arada yeniden açıldıysa
      // (React'in kontrol amaçlı ikinci çalıştırması dahil) kayıt yerinde kalır.
      // Hemen geri alınsaydı tarayıcının eşzamansız "geri" adımı yeni açılan
      // katmanı da kapatırdı.
      window.setTimeout(() => {
        if (depth < mine && (window.history.state as Entry)?.layer === mine) {
          window.history.back();
        }
      }, 0);
    };
  }, [open, enabled]);
}
