/* Sahifa reader runtime: pagination (LTR + RTL), gestures, search highlights.
   Injected into every chapter; settings arrive in window.__SAHIFA__. */
(function () {
  'use strict';
  var B = window.SahifaBridge || {
    onProgress: function () {}, onReady: function () {}, toggleChrome: function () {},
    nextChapter: function () {}, prevChapter: function () {}, onSelection: function () {},
    locate: function () { return '[]'; }
  };
  var de = document.documentElement;
  var S = window.__SAHIFA__ || { mode: 'scroll' };
  var R = (window.Reader = {});
  var origDir = de.getAttribute('dir');
  var lastFraction = 0;

  function W() { return window.innerWidth || de.clientWidth; }
  function paged() { return S.mode === 'paged'; }
  function isRtl() { return getComputedStyle(de).direction === 'rtl'; }

  R.apply = function (s, keep) {
    var f = keep === false ? null : R.fraction();
    S = s;
    var st = de.style;
    st.setProperty('--fs', s.fs + 'px');
    st.setProperty('--lh', String(s.lh));
    st.setProperty('--margin', s.margin + 'px');
    st.setProperty('--fg', s.fg);
    st.setProperty('--bg', s.bg);
    st.setProperty('--link', s.link);
    if (s.font) { st.setProperty('--font', s.font); de.classList.add('font-custom'); }
    else de.classList.remove('font-custom');
    de.classList.toggle('themed', !!s.themed);
    de.classList.toggle('justify', !!s.justify);
    de.classList.toggle('paged', paged());
    sizePages();
    de.classList.toggle('fixed', !!s.fixed);
    if (s.dir === 'rtl' || s.dir === 'ltr') de.setAttribute('dir', s.dir);
    else if (origDir) de.setAttribute('dir', origDir);
    else de.removeAttribute('dir');
    nextButton();
    if (f !== null) requestAnimationFrame(function () { R.goFraction(f); });
  };

  /* vh units are unreliable inside an embedded WebView (they can resolve to 0
     before the first layout), so page geometry is set in pixels. */
  function sizePages() {
    var st = de.style;
    if (paged()) {
      st.setProperty('height', window.innerHeight + 'px', 'important');
      st.setProperty('column-width', W() + 'px', 'important');
      st.setProperty('--page-h', window.innerHeight + 'px');
    } else {
      st.removeProperty('height');
      st.removeProperty('column-width');
    }
  }

  function nextButton() {
    var b = document.getElementById('sahifa-next');
    var want = !paged() && S.hasNext && document.body;
    if (want && !b) {
      b = document.createElement('button');
      b.id = 'sahifa-next';
      b.textContent = S.nextLabel || '→';
      b.addEventListener('click', function (e) { e.stopPropagation(); B.nextChapter(); });
      document.body.appendChild(b);
    } else if (!want && b) b.remove();
  }

  R.pages = function () {
    if (!paged()) return 1;
    return Math.max(1, Math.round(de.scrollWidth / W()));
  };
  R.page = function () {
    return paged() ? Math.round(Math.abs(window.scrollX) / W()) : 0;
  };
  R.fraction = function () {
    if (paged()) {
      var n = R.pages();
      return n > 1 ? R.page() / (n - 1) : 0;
    }
    var max = Math.max(1, de.scrollHeight - window.innerHeight);
    return Math.min(1, Math.max(0, window.scrollY / max));
  };
  R.goPage = function (n) {
    n = Math.max(0, Math.min(R.pages() - 1, n));
    window.scrollTo({ left: (isRtl() ? -1 : 1) * n * W(), top: 0, behavior: 'instant' });
    report();
  };
  R.goFraction = function (f) {
    if (paged()) R.goPage(Math.round(f * (R.pages() - 1)));
    else {
      window.scrollTo({ left: 0, top: f * Math.max(0, de.scrollHeight - window.innerHeight), behavior: 'instant' });
      report();
    }
  };
  R.goRect = function (r) {
    if (paged()) {
      var cx = (r.left + r.right) / 2 + window.scrollX;
      var p = isRtl() ? Math.floor(-cx / W()) + 1 : Math.floor(cx / W());
      R.goPage(p);
    } else {
      window.scrollTo({ left: 0, top: r.top + window.scrollY - window.innerHeight / 3, behavior: 'instant' });
      report();
    }
  };
  R.goAnchor = function (id) {
    var el = document.getElementById(id) || document.getElementsByName(id)[0];
    if (el) R.goRect(el.getBoundingClientRect());
    else R.goFraction(0);
  };
  R.next = function () {
    if (paged()) {
      if (R.page() < R.pages() - 1) R.goPage(R.page() + 1); else B.nextChapter();
    } else if (window.scrollY + window.innerHeight >= de.scrollHeight - 4) B.nextChapter();
    else window.scrollBy({ top: window.innerHeight - 64, behavior: 'smooth' });
  };
  R.prev = function () {
    if (paged()) {
      if (R.page() > 0) R.goPage(R.page() - 1); else B.prevChapter();
    } else if (window.scrollY <= 0) B.prevChapter();
    else window.scrollBy({ top: -(window.innerHeight - 64), behavior: 'smooth' });
  };

  var reportTimer = null;
  function report() {
    lastFraction = R.fraction();
    if (reportTimer) return;
    reportTimer = setTimeout(function () {
      reportTimer = null;
      lastFraction = R.fraction();
      B.onProgress(lastFraction, R.page(), R.pages());
    }, 120);
  }
  window.addEventListener('scroll', report, { passive: true });

  /* ---------- gestures ---------- */
  var sx = null, sy = 0, stime = 0, swiped = false;
  document.addEventListener('touchstart', function (e) {
    if (e.touches.length !== 1) { sx = null; return; }
    sx = e.touches[0].clientX; sy = e.touches[0].clientY; stime = Date.now(); swiped = false;
  }, { passive: true });
  document.addEventListener('touchend', function (e) {
    if (sx === null || !paged()) return;
    var t = e.changedTouches[0], dx = t.clientX - sx, dy = t.clientY - sy;
    if (hasSelection()) return;
    if (Math.abs(dx) > 40 && Math.abs(dx) > Math.abs(dy) * 1.2 && Date.now() - stime < 900) {
      swiped = true;
      var forward = isRtl() ? dx > 0 : dx < 0;
      if (forward) R.next(); else R.prev();
    }
  }, { passive: true });
  document.addEventListener('click', function (e) {
    if (swiped) { swiped = false; return; }
    if (e.target.closest && e.target.closest('a[href], button, input, select, textarea, audio, video')) return;
    if (hasSelection()) return;
    var x = e.clientX, w = W();
    if (paged() && x < w * 0.28) { if (isRtl()) R.next(); else R.prev(); }
    else if (paged() && x > w * 0.72) { if (isRtl()) R.prev(); else R.next(); }
    else B.toggleChrome();
  });
  function hasSelection() {
    var s = window.getSelection && window.getSelection();
    return !!(s && !s.isCollapsed && String(s).trim());
  }
  var selTimer = null;
  document.addEventListener('selectionchange', function () {
    clearTimeout(selTimer);
    selTimer = setTimeout(function () {
      var s = window.getSelection();
      B.onSelection(s ? String(s).trim() : '');
    }, 250);
  });

  /* ---------- search & highlights (matching done by the Rust core) ---------- */
  function textNodes() {
    var nodes = [], texts = [];
    if (!document.body) return { nodes: nodes, texts: texts };
    var w = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT, {
      acceptNode: function (n) {
        var p = n.parentNode && n.parentNode.nodeName;
        return /^(script|style|noscript|button)$/i.test(p || '') ? NodeFilter.FILTER_REJECT : NodeFilter.FILTER_ACCEPT;
      }
    });
    var n;
    while ((n = w.nextNode())) { nodes.push(n); texts.push(n.nodeValue); }
    return { nodes: nodes, texts: texts };
  }
  function rangesFor(q) {
    var t = textNodes();
    var res = [];
    try { res = JSON.parse(B.locate(JSON.stringify(t.texts), q) || '[]'); } catch (e) { res = []; }
    return res.map(function (r) {
      var rg = document.createRange();
      rg.setStart(t.nodes[r[0]], r[1]);
      rg.setEnd(t.nodes[r[2]], r[3]);
      return rg;
    });
  }
  var hasHL = !!(window.CSS && CSS.highlights && window.Highlight);
  function paint(name, ranges) {
    if (hasHL) {
      if (ranges.length) CSS.highlights.set(name, new Highlight(ranges[0]));
      var h = CSS.highlights.get(name);
      if (h) { h.clear(); ranges.forEach(function (r) { h.add(r); }); }
      return;
    }
    // Fallback: wrap single-node ranges, last first so offsets stay valid.
    unpaint(name);
    ranges.slice().reverse().forEach(function (r) {
      if (r.startContainer !== r.endContainer) return;
      var m = document.createElement('mark');
      m.className = name;
      try { r.surroundContents(m); } catch (e) {}
    });
  }
  function unpaint(name) {
    if (hasHL) { CSS.highlights.delete(name); return; }
    document.querySelectorAll('mark.' + name).forEach(function (m) {
      var p = m.parentNode;
      while (m.firstChild) p.insertBefore(m.firstChild, m);
      p.removeChild(m);
      p.normalize();
    });
  }
  R.find = function (q, occ) {
    var rs = rangesFor(q);
    paint('sahifa-hit', rs);
    var cur = rs[Math.min(occ || 0, rs.length - 1)];
    if (cur) { paint('sahifa-current', [cur]); R.goRect(cur.getBoundingClientRect()); }
    return rs.length;
  };
  R.clearFind = function () { unpaint('sahifa-hit'); unpaint('sahifa-current'); };
  R.userHighlights = function (list) {
    var all = [];
    (list || []).forEach(function (q) { all = all.concat(rangesFor(q)); });
    if (all.length) paint('sahifa-user', all); else unpaint('sahifa-user');
  };
  R.selectionText = function () { return hasSelection() ? String(window.getSelection()).trim() : ''; };

  /* ---------- boot ---------- */
  var started = false;
  function start() {
    if (started) return;
    started = true;
    R.apply(S, false);
    var st = S.start || {};
    requestAnimationFrame(function () {
      if (st.find) R.find(st.find, st.occ || 0);
      else if (st.anchor) R.goAnchor(st.anchor);
      else R.goFraction(st.fraction || 0);
      if (S.user && S.user.length) R.userHighlights(S.user);
      B.onReady(R.pages());
    });
  }
  function relayout() { if (started) { var f = lastFraction; sizePages(); R.goFraction(f); } }
  if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', start); else start();
  window.addEventListener('load', function () { start(); relayout(); });
  if (document.fonts && document.fonts.ready) document.fonts.ready.then(relayout);
  window.addEventListener('resize', function () { setTimeout(relayout, 60); });
})();
