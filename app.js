(function () {
  let pendingCartOpen = false;

  function sendEvent(payload) {
    const body = JSON.stringify(Object.assign({
      page_path: window.location.pathname + window.location.search,
      metadata: {}
    }, payload));

    if (navigator.sendBeacon) {
      const blob = new Blob([body], { type: "application/json" });
      if (navigator.sendBeacon("/events", blob)) return;
    }

    fetch("/events", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: body,
      keepalive: true
    }).catch(function () {});
  }

  function trackedPayload(element, eventName) {
    return {
      event_name: eventName,
      source: element.dataset.source || "",
      target_type: element.dataset.targetType || "",
      target_id: element.dataset.targetId || "",
      metadata: {
        text: (element.textContent || "").trim().slice(0, 160),
        action: element.dataset.action || "",
        href: element.getAttribute("href") || "",
        tag: element.tagName.toLowerCase()
      }
    };
  }

  function trackClick(event) {
    const element = event.target.closest("[data-track-click]");
    if (!element) return;
    const eventName = element.dataset.trackClick;
    if (!eventName) return;
    sendEvent(trackedPayload(element, eventName));
  }

  function trackSearch(form, source) {
    const data = new FormData(form);
    const query = (data.get("q") || "").toString().trim();
    const genre = (data.get("genre") || "").toString();
    const condition = (data.get("condition") || "").toString();
    const format = (data.get("format") || "").toString();
    const sort = (data.get("sort") || "").toString();
    const maxPrice = (data.get("max_price") || "").toString();

    sendEvent({
      event_name: "catalog_searched",
      source: source,
      target_type: "search",
      target_id: query || genre || condition || format || sort || maxPrice || "catalog",
      metadata: {
        q: query,
        genre: genre,
        condition: condition,
        format: format,
        sort: sort,
        max_price: maxPrice
      }
    });
  }

  function cartDrawer() {
    return document.getElementById("cartDrawer");
  }

  function isInsideCartDrawer(element) {
    return element && element.closest && element.closest("#cartDrawer");
  }

  function syncCartCount() {
    const drawer = cartDrawer();
    const badge = document.querySelector("[data-cart-count]");
    if (drawer && badge) {
      const oldCount = badge.textContent || "0";
      const newCount = drawer.dataset.cartCount || "0";
      badge.textContent = newCount;
      if (parseInt(newCount) > parseInt(oldCount)) {
        badge.classList.remove("badge-pop");
        void badge.offsetWidth; // Trigger reflow to restart animation
        badge.classList.add("badge-pop");
      }
    }
  }

  function openCart() {
    const drawer = cartDrawer();
    if (!drawer) return;
    drawer.classList.add("is-open");
    drawer.setAttribute("aria-hidden", "false");
    document.body.classList.add("cart-open");
    const toggle = document.querySelector(".cart-toggle");
    if (toggle) toggle.setAttribute("aria-expanded", "true");
  }

  function openCartSoon() {
    [50, 200, 600].forEach(function (delay) {
      window.setTimeout(function () {
        syncCartCount();
        if (pendingCartOpen) openCart();
      }, delay);
    });
    window.setTimeout(function () {
      pendingCartOpen = false;
    }, 750);
  }

  function closeCart() {
    const drawer = cartDrawer();
    if (!drawer) return;
    drawer.classList.remove("is-open");
    drawer.setAttribute("aria-hidden", "true");
    document.body.classList.remove("cart-open");
    const toggle = document.querySelector(".cart-toggle");
    if (toggle) toggle.setAttribute("aria-expanded", "false");
  }

  function initHeroCarousel() {
    const track = document.getElementById("heroCarouselTrack");
    if (!track) return;
    const slides = Array.from(track.querySelectorAll(".hero-slide"));
    const dots = Array.from(document.querySelectorAll("#heroCarouselDots .dot"));
    const prevBtn = document.getElementById("heroPrevBtn");
    const nextBtn = document.getElementById("heroNextBtn");
    if (slides.length === 0) return;

    let activeIndex = 0;
    let timer = null;

    function showSlide(index) {
      slides[activeIndex].classList.remove("active");
      if (dots[activeIndex]) dots[activeIndex].classList.remove("active");
      activeIndex = (index + slides.length) % slides.length;
      slides[activeIndex].classList.add("active");
      if (dots[activeIndex]) dots[activeIndex].classList.add("active");
    }

    function startTimer() {
      window.clearInterval(timer);
      timer = window.setInterval(function () {
        showSlide(activeIndex + 1);
      }, 6000);
    }

    if (prevBtn) {
      prevBtn.addEventListener("click", function () {
        showSlide(activeIndex - 1);
        startTimer();
      });
    }
    if (nextBtn) {
      nextBtn.addEventListener("click", function () {
        showSlide(activeIndex + 1);
        startTimer();
      });
    }
    dots.forEach(function (dot, index) {
      dot.addEventListener("click", function () {
        showSlide(index);
        startTimer();
      });
    });
    startTimer();
  }

  document.addEventListener("click", function (event) {
    trackClick(event);

    if (event.target.closest(".close-cart")) {
      closeCart();
      return;
    }
    const drawer = cartDrawer();
    if (drawer && event.target === drawer) {
      closeCart();
      return;
    }
    if (event.target.closest('[data-action="add"], [data-action="buy-now-card"]')) {
      pendingCartOpen = true;
      openCartSoon();
      return;
    }
    const bookCard = event.target.closest("[data-book-url]");
    if (bookCard && !event.target.closest("button, a, input, select, textarea")) {
      window.location.href = bookCard.dataset.bookUrl;
    }
  });

  document.addEventListener("keydown", function (event) {
    if (event.key === "Escape") closeCart();
  });

  document.addEventListener("input", function (event) {
    if (event.target.id !== "priceFilter") return;
    const priceValue = document.getElementById("priceValue");
    if (priceValue) priceValue.textContent = "$" + event.target.value;
  });

  document.addEventListener("submit", function (event) {
    if (!event.target.matches("#headerSearchForm")) {
      return;
    }
    trackSearch(event.target, "header.search");
    if (document.getElementById("catalogResults")) {
      return;
    }
    event.preventDefault();
    const params = new URLSearchParams();
    const query = document.getElementById("headerSearchInput");
    const genre = document.getElementById("headerGenreSelect");
    if (query && query.value.trim()) params.set("q", query.value.trim());
    if (genre && genre.value && genre.value !== "All") params.set("genre", genre.value);
    const search = params.toString();
    window.location.href = "/" + (search ? "?" + search : "") + "#catalog";
  }, true);

  document.addEventListener("submit", function (event) {
    if (!event.target.matches("#catalogFilters")) return;
    trackSearch(event.target, "catalog.filters");
  }, true);

  document.body.addEventListener("htmx:beforeRequest", function (event) {
    const source = event.detail && event.detail.elt;
    if (!source || !source.closest) return;
    if (isInsideCartDrawer(source)) {
      pendingCartOpen = true;
    }
    const form = source.matches("#catalogFilters") ? source : source.closest("#catalogFilters");
    if (!form) return;
    trackSearch(form, "catalog.filters");
  });

  document.body.addEventListener("htmx:afterSwap", function (event) {
    syncCartCount();
    if ((event.detail && event.detail.target && event.detail.target.id === "cartDrawer") || pendingCartOpen) {
      openCart();
    }
  });

  document.body.addEventListener("htmx:afterRequest", openCartSoon);

  document.addEventListener("DOMContentLoaded", function () {
    syncCartCount();
    initHeroCarousel();
  });

  document.addEventListener("click", function (event) {
    const tabButton = event.target.closest("[data-tab-target]");
    if (!tabButton) return;
    const container = tabButton.closest(".details-tabs-section");
    if (!container) return;
    container.querySelectorAll(".modal-tab-button").forEach(function (button) {
      button.classList.toggle("is-active", button === tabButton);
    });
    container.querySelectorAll(".modal-tab-content").forEach(function (content) {
      content.classList.toggle("is-active", content.id === tabButton.dataset.tabTarget);
    });
  });
})();
