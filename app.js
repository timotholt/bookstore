(function () {
  let pendingCartOpen = false;

  function cartDrawer() {
    return document.getElementById("cartDrawer");
  }

  function syncCartCount() {
    const drawer = cartDrawer();
    const badge = document.querySelector("[data-cart-count]");
    if (drawer && badge) {
      badge.textContent = drawer.dataset.cartCount || "0";
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
    if (event.target.closest(".cart-toggle")) {
      openCart();
      return;
    }
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
    if (!event.target.matches("#headerSearchForm") || document.getElementById("catalogResults")) {
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

  document.body.addEventListener("htmx:afterSwap", function (event) {
    syncCartCount();
    if (event.detail && event.detail.target && event.detail.target.id === "cartDrawer") {
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
