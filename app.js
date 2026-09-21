(function () {
  let pendingCartOpen = false;
  let cartWasOpenBeforeSwap = false;
  let drawerItemsScrollTop = 0;
  let cartPageScrollY = null;
  let catalogRefreshTimer = null;
  let catalogAbortController = null;
  let catalogRequestSeq = 0;
  const prefetchedBookUrls = new Set();
  const bookPrefetchTimers = new WeakMap();

  function prefetchBook(card) {
    if (!card || !card.dataset.bookUrl || prefetchedBookUrls.has(card.dataset.bookUrl)) return;
    if (navigator.connection && navigator.connection.saveData) return;

    const url = card.dataset.bookUrl;
    const link = document.createElement("link");
    link.rel = "prefetch";
    link.href = url;
    document.head.appendChild(link);
    prefetchedBookUrls.add(url);
  }

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
    const minRating = (data.get("min_rating") || "").toString();
    const listings = data.getAll("listing").map(function (value) {
      return value.toString();
    });

    sendEvent({
      event_name: "catalog_searched",
      source: source,
      target_type: "search",
      target_id: query || genre || condition || format || sort || maxPrice || minRating || "catalog",
      metadata: {
        q: query,
        genre: genre,
        condition: condition,
        listing: listings,
        format: format,
        sort: sort,
        max_price: maxPrice,
        min_rating: minRating
      }
    });
  }

  function cartDrawer() {
    return document.getElementById("cartDrawer");
  }

  function isInsideCartDrawer(element) {
    return element && element.closest && element.closest("#cartDrawer");
  }

  function rememberDrawerScroll() {
    const items = document.querySelector("#cartDrawer .cart-items");
    drawerItemsScrollTop = items ? items.scrollTop : 0;
  }

  function restoreDrawerScroll() {
    const items = document.querySelector("#cartDrawer .cart-items");
    if (items && drawerItemsScrollTop > 0) {
      items.scrollTop = drawerItemsScrollTop;
    }
  }

  function rememberCartPageScroll(source) {
    if (!source || !source.closest || !source.closest("#cartPageMain")) return;
    cartPageScrollY = window.scrollY;
  }

  function restoreCartPageScroll() {
    if (cartPageScrollY === null) return;
    const cartMain = document.getElementById("cartPageMain");
    const hasActiveCartLines = Boolean(document.querySelector("#cartPageMain [data-cart-line]"));
    if (cartMain && !hasActiveCartLines) {
      window.scrollTo(0, cartMain.offsetTop);
    } else {
      window.scrollTo(0, cartPageScrollY);
    }
    cartPageScrollY = null;
  }

  function syncCartCount() {
    const drawer = cartDrawer();
    const source = drawer || document.getElementById("cartPageMain");
    const badges = Array.from(document.querySelectorAll("[data-cart-count-badge]"));
    if (source && badges.length > 0) {
      const newCount = source.dataset.cartCount || "0";
      badges.forEach(function (badge) {
        const oldCount = badge.textContent || "0";
        badge.textContent = newCount;
        if (parseInt(newCount) > parseInt(oldCount)) {
          badge.classList.remove("badge-pop");
          void badge.offsetWidth; // Trigger reflow to restart animation
          badge.classList.add("badge-pop");
        }
      });
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

  function openCartWithoutAnimation() {
    const drawer = cartDrawer();
    if (!drawer) return;
    const panel = drawer.querySelector(".cart-panel");
    if (panel) panel.classList.add("no-drawer-animation");
    openCart();
    if (panel) {
      window.requestAnimationFrame(function () {
        panel.classList.remove("no-drawer-animation");
      });
    }
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

  function appendCatalogParam(params, key, value) {
    const normalized = (value || "").toString().trim();
    if (!normalized || normalized === "All") return;
    if (key === "max_price" && normalized === "499") return;
    if (key === "sort" && normalized === "popular") return;
    params.append(key, normalized);
  }

  function catalogSearchParams() {
    const form = document.getElementById("catalogFilters");
    if (!form) return null;
    syncListingFilter();
    const data = new FormData(form);
    const params = new URLSearchParams();

    data.forEach(function (value, key) {
      appendCatalogParam(params, key, value);
    });

    const sort = document.getElementById("sortSelect");
    if (sort) {
      params.delete("sort");
      appendCatalogParam(params, "sort", sort.value);
    }

    return params;
  }

  function syncListingFilter() {
    const form = document.getElementById("catalogFilters");
    if (!form) return;
    const hidden = document.getElementById("listingFilter");
    if (!hidden) return;

    const selectedListings = Array.from(form.querySelectorAll("[data-listing-option]:checked")).map(function (input) {
      return input.value;
    });
    hidden.value = selectedListings.length === 1 ? selectedListings[0] : "";
  }

  function syncRatingStars() {
    const form = document.getElementById("catalogFilters");
    if (!form) return;
    const checked = form.querySelector('input[name="min_rating"]:checked');
    const value = checked ? checked.value : "";
    const activeStar = value ? parseInt(value, 10) : 1;

    form.querySelectorAll(".rating-star-option").forEach(function (option, index) {
      const starNumber = index + 1;
      const input = option.querySelector('input[name="min_rating"]');
      option.classList.toggle("is-filled", starNumber <= activeStar);
      option.classList.toggle("is-selected", Boolean(input && input.checked));
    });
  }

  function refreshCatalogResults(source, delay) {
    const form = document.getElementById("catalogFilters");
    const target = document.getElementById("catalogResults");
    const params = catalogSearchParams();
    if (!form || !target || !params) return;

    window.clearTimeout(catalogRefreshTimer);
    catalogRefreshTimer = window.setTimeout(function () {
      const query = params.toString();
      const catalogUrl = "/catalog" + (query ? "?" + query : "");
      const searchUrl = "/search" + (query ? "?" + query : "");

      if (catalogAbortController) {
        catalogAbortController.abort();
      }
      catalogAbortController = new AbortController();
      const requestSeq = ++catalogRequestSeq;
      target.classList.add("is-loading");

      fetch(catalogUrl, {
        headers: { "HX-Request": "true" },
        signal: catalogAbortController.signal
      })
        .then(function (response) {
          if (!response.ok) throw new Error("Catalog refresh failed");
          return response.text();
        })
        .then(function (html) {
          if (requestSeq !== catalogRequestSeq) return;
          const currentTarget = document.getElementById("catalogResults");
          if (currentTarget) currentTarget.outerHTML = html;
          window.history.replaceState({}, "", searchUrl);
          trackSearch(form, source || "catalog.filters");
        })
        .catch(function (error) {
          if (requestSeq === catalogRequestSeq && error.name !== "AbortError") {
            console.error(error);
            target.classList.remove("is-loading");
          }
        })
        .finally(function () {
          if (requestSeq !== catalogRequestSeq) return;
          const currentTarget = document.getElementById("catalogResults");
          if (currentTarget) currentTarget.classList.remove("is-loading");
        });
    }, delay || 0);
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

  document.addEventListener("pointerover", function (event) {
    if (event.pointerType !== "mouse") return;
    const card = event.target.closest("[data-book-url]");
    if (!card || (event.relatedTarget && card.contains(event.relatedTarget))) return;

    const timer = window.setTimeout(function () {
      prefetchBook(card);
      bookPrefetchTimers.delete(card);
    }, 150);
    bookPrefetchTimers.set(card, timer);
  });

  document.addEventListener("pointerout", function (event) {
    if (event.pointerType !== "mouse") return;
    const card = event.target.closest("[data-book-url]");
    if (!card || (event.relatedTarget && card.contains(event.relatedTarget))) return;

    const timer = bookPrefetchTimers.get(card);
    if (timer) {
      window.clearTimeout(timer);
      bookPrefetchTimers.delete(card);
    }
  });

  document.addEventListener("keydown", function (event) {
    if (event.key === "Escape") closeCart();
  });

  document.addEventListener("input", function (event) {
    const catalogForm = event.target.closest("#catalogFilters");
    if (event.target.id === "priceFilter") {
      const priceValue = document.getElementById("priceValue");
      if (priceValue) priceValue.textContent = "$" + event.target.value;
    }
    if (!catalogForm) return;
    if (event.target.matches('input[type="search"], input[type="range"]')) {
      refreshCatalogResults("catalog.filters", event.target.type === "range" ? 120 : 250);
    }
  });

  document.addEventListener("change", function (event) {
    if (event.target.closest("#catalogFilters")) {
      if (event.target.matches("[data-listing-option]")) syncListingFilter();
      if (event.target.matches('input[name="min_rating"]')) syncRatingStars();
      refreshCatalogResults("catalog.filters", 0);
      return;
    }
    if (event.target.matches("#sortSelect")) {
      refreshCatalogResults("catalog.sort", 0);
    }
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
    window.location.href = "/search" + (search ? "?" + search : "");
  }, true);

  document.addEventListener("submit", function (event) {
    if (!event.target.matches("#catalogFilters")) return;
    event.preventDefault();
    refreshCatalogResults("catalog.filters", 0);
  }, true);

  document.body.addEventListener("htmx:beforeRequest", function (event) {
    const source = event.detail && event.detail.elt;
    if (!source || !source.closest) return;
    if (isInsideCartDrawer(source)) {
      rememberDrawerScroll();
      if (document.body.classList.contains("cart-open")) {
        document.body.classList.add("cart-drawer-updating");
      }
    }
    rememberCartPageScroll(source);
    const form = source.matches("#catalogFilters") ? source : source.closest("#catalogFilters");
    if (!form) return;
    trackSearch(form, "catalog.filters");
  });

  document.body.addEventListener("htmx:beforeSwap", function (event) {
    const target = event.detail && event.detail.target;
    cartWasOpenBeforeSwap = Boolean(target && target.id === "cartDrawer" && target.classList.contains("is-open"));
    if (cartWasOpenBeforeSwap) {
      rememberDrawerScroll();
      document.body.classList.add("cart-drawer-updating");
    }
  });

  document.body.addEventListener("htmx:afterSwap", function (event) {
    syncCartCount();
    if ((event.detail && event.detail.target && event.detail.target.id === "cartDrawer") || pendingCartOpen) {
      if (cartWasOpenBeforeSwap) {
        openCartWithoutAnimation();
        window.requestAnimationFrame(restoreDrawerScroll);
      } else {
        openCart();
      }
    }
    if (event.detail && event.detail.target && event.detail.target.id === "cartPageMain") {
      window.requestAnimationFrame(restoreCartPageScroll);
    }
    cartWasOpenBeforeSwap = false;
  });

  document.body.addEventListener("htmx:afterSettle", function () {
    document.body.classList.remove("cart-drawer-updating");
  });

  document.body.addEventListener("htmx:afterRequest", function () {
    if (pendingCartOpen) openCartSoon();
  });

  document.addEventListener("DOMContentLoaded", function () {
    syncCartCount();
    initHeroCarousel();
    syncListingFilter();
    syncRatingStars();
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
