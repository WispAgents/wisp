// F-phone-6 (v1 index.html's first script, homescreen.js): wisp opened from an iPhone's Home Screen (or installed
// from Chrome) runs full screen, so the kit keeps the page out of the status bar, the notch and the home indicator
// (app-stack.css, html[data-standalone]). Set before any module, so the first frame is already laid out for it. A
// browser tab never gets it, nor the native apps (Capacitor's bridge, the Android app's `wispAndroid` and its
// `wispMobile` user agent): they lay out their own edges.

/** index.html's inline script, as a string (the test runs this very text). */
export const STANDALONE_CHECK = `try{var n=navigator,w=window;var c=w.Capacitor;if(!(c&&c.isNativePlatform&&c.isNativePlatform())&&!w.wispAndroid&&!/\\bwispMobile\\b/.test(n.userAgent)&&(n.standalone===true||matchMedia('(display-mode: standalone)').matches))document.documentElement.setAttribute('data-standalone','')}catch(e){}`;
