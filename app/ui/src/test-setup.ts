// Gemeinsame Vorbereitung der Komponententests (jsdom).
//
// jsdom kennt `matchMedia` nicht, uPlot ruft es beim Laden des Moduls auf. Der Ersatz meldet
// "keine Übereinstimmung" (dunkles Thema, normale Pixeldichte).

if (typeof window !== 'undefined' && typeof window.matchMedia !== 'function') {
  window.matchMedia = (query: string): MediaQueryList =>
    ({
      matches: false,
      media: query,
      onchange: null,
      addEventListener: () => {},
      removeEventListener: () => {},
      addListener: () => {},
      removeListener: () => {},
      dispatchEvent: () => false,
    }) as MediaQueryList;
}
