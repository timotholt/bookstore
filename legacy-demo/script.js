const books = [
  {
    id: "b001",
    title: "The Night Circus",
    author: "Erin Morgenstern",
    genre: "Fiction",
    price: 9.5,
    condition: "Very Good",
    format: "Trade Paperback",
    year: 2012,
    stock: 3,
    isbn: "9780307744432",
    tags: ["magical", "atmospheric", "romance"],
    notes: "Clean pages, faint spine crease, soft corner wear.",
    color: "#1a2e26",
    staffPick: true,
    newArrival: false,
    aspectRatio: 0.65,
    sealStyle: "circle",
    sealText: "DB"
  },
  {
    id: "b002",
    title: "Kitchen Confidential",
    author: "Anthony Bourdain",
    genre: "Memoir",
    price: 8.75,
    condition: "Good",
    format: "Paperback",
    year: 2007,
    stock: 2,
    isbn: "9780060899226",
    tags: ["food", "memoir", "sharp"],
    notes: "Readable copy with shelf scuffs and a small owner stamp.",
    color: "#6b1827",
    staffPick: false,
    newArrival: true,
    aspectRatio: 0.72,
    sealStyle: "square",
    sealText: "USED"
  },
  {
    id: "b003",
    title: "Dune",
    author: "Frank Herbert",
    genre: "Science Fiction",
    price: 11.25,
    condition: "Very Good",
    format: "Paperback",
    year: 2019,
    stock: 4,
    isbn: "9780441172719",
    tags: ["classic", "desert", "politics"],
    notes: "Tight binding, light page tanning, no markings.",
    color: "#a7482f",
    staffPick: true,
    newArrival: true,
    aspectRatio: 0.68,
    sealStyle: "diamond",
    sealText: "1st"
  },
  {
    id: "b004",
    title: "The Secret History",
    author: "Donna Tartt",
    genre: "Mystery",
    price: 10.5,
    condition: "Good",
    format: "Trade Paperback",
    year: 2004,
    stock: 2,
    isbn: "9781400031702",
    tags: ["campus", "literary", "dark"],
    notes: "Some margin notes in pencil, square spine.",
    color: "#233e54",
    staffPick: true,
    newArrival: false,
    aspectRatio: 0.75,
    sealStyle: "none",
    sealText: ""
  },
  {
    id: "b005",
    title: "Braiding Sweetgrass",
    author: "Robin Wall Kimmerer",
    genre: "Nature",
    price: 12.75,
    condition: "Like New",
    format: "Paperback",
    year: 2020,
    stock: 3,
    isbn: "9781571313560",
    tags: ["ecology", "essays", "indigenous"],
    notes: "Gift-quality copy with a tiny back-cover nick.",
    color: "#284234",
    staffPick: true,
    newArrival: false,
    aspectRatio: 0.70,
    sealStyle: "circle",
    sealText: "CRISP"
  },
  {
    id: "b006",
    title: "The Thursday Murder Club",
    author: "Richard Osman",
    genre: "Mystery",
    price: 7.95,
    condition: "Very Good",
    format: "Paperback",
    year: 2021,
    stock: 5,
    isbn: "9781984880987",
    tags: ["cozy", "witty", "series"],
    notes: "Bright cover, minor shelf rubbing.",
    color: "#b48c34",
    staffPick: false,
    newArrival: true,
    aspectRatio: 0.62,
    sealStyle: "diamond",
    sealText: "GOLD"
  },
  {
    id: "b007",
    title: "A Gentleman in Moscow",
    author: "Amor Towles",
    genre: "Historical Fiction",
    price: 9.25,
    condition: "Very Good",
    format: "Trade Paperback",
    year: 2019,
    stock: 3,
    isbn: "9780143110439",
    tags: ["elegant", "hotel", "historical"],
    notes: "Smooth spine, lightly bumped lower corner.",
    color: "#263e52",
    staffPick: true,
    newArrival: false,
    aspectRatio: 0.74,
    sealStyle: "square",
    sealText: "DB"
  },
  {
    id: "b008",
    title: "The Hobbit",
    author: "J.R.R. Tolkien",
    genre: "Fantasy",
    price: 6.95,
    condition: "Fair",
    format: "Paperback",
    year: 2001,
    stock: 6,
    isbn: "9780618260300",
    tags: ["classic", "adventure", "quest"],
    notes: "Well-loved reading copy with creases and tanned pages.",
    color: "#4e5933",
    staffPick: false,
    newArrival: false,
    aspectRatio: 0.60,
    sealStyle: "none",
    sealText: ""
  },
  {
    id: "b009",
    title: "Salt Fat Acid Heat",
    author: "Samin Nosrat",
    genre: "Cookbooks",
    price: 18.5,
    condition: "Very Good",
    format: "Hardcover",
    year: 2017,
    stock: 1,
    isbn: "9781476753836",
    tags: ["cooking", "illustrated", "technique"],
    notes: "No food stains, jacket has a short closed tear.",
    color: "#ab5433",
    staffPick: true,
    newArrival: true,
    aspectRatio: 0.82,
    sealStyle: "circle",
    sealText: "COOK"
  },
  {
    id: "b010",
    title: "The Warmth of Other Suns",
    author: "Isabel Wilkerson",
    genre: "History",
    price: 13.95,
    condition: "Good",
    format: "Paperback",
    year: 2011,
    stock: 2,
    isbn: "9780679763888",
    tags: ["history", "migration", "america"],
    notes: "Strong reading copy with several dog-eared pages.",
    color: "#543928",
    staffPick: false,
    newArrival: false,
    aspectRatio: 0.71,
    sealStyle: "none",
    sealText: ""
  },
  {
    id: "b011",
    title: "Project Hail Mary",
    author: "Andy Weir",
    genre: "Science Fiction",
    price: 10.95,
    condition: "Like New",
    format: "Paperback",
    year: 2022,
    stock: 4,
    isbn: "9780593135228",
    tags: ["space", "clever", "survival"],
    notes: "Crisp copy with a clean unbroken spine.",
    color: "#214e63",
    staffPick: false,
    newArrival: true,
    aspectRatio: 0.66,
    sealStyle: "circle",
    sealText: "NEW"
  },
  {
    id: "b012",
    title: "The Vanishing Half",
    author: "Brit Bennett",
    genre: "Fiction",
    price: 8.95,
    condition: "Very Good",
    format: "Paperback",
    year: 2021,
    stock: 3,
    isbn: "9780525536963",
    tags: ["family", "identity", "literary"],
    notes: "Clean text block, faint bend on front cover.",
    color: "#73334c",
    staffPick: false,
    newArrival: false,
    aspectRatio: 0.73,
    sealStyle: "diamond",
    sealText: "DB"
  },
  {
    id: "b013",
    title: "World of Wonders",
    author: "Aimee Nezhukumatathil",
    genre: "Nature",
    price: 7.5,
    condition: "Very Good",
    format: "Hardcover",
    year: 2020,
    stock: 2,
    isbn: "9781571313652",
    tags: ["essays", "nature", "lyrical"],
    notes: "Small remainder mark, otherwise fresh.",
    color: "#2b5c5a",
    staffPick: true,
    newArrival: false,
    aspectRatio: 0.78,
    sealStyle: "square",
    sealText: "WILD"
  },
  {
    id: "b014",
    title: "The Book Thief",
    author: "Markus Zusak",
    genre: "Historical Fiction",
    price: 6.75,
    condition: "Good",
    format: "Paperback",
    year: 2007,
    stock: 5,
    isbn: "9780375842207",
    tags: ["historical", "young adult", "moving"],
    notes: "Moderate cover wear, clean pages.",
    color: "#3f4b54",
    staffPick: false,
    newArrival: false,
    aspectRatio: 0.63,
    sealStyle: "none",
    sealText: ""
  },
  {
    id: "b015",
    title: "The Midnight Library",
    author: "Matt Haig",
    genre: "Fiction",
    price: 8.25,
    condition: "Like New",
    format: "Hardcover",
    year: 2020,
    stock: 2,
    isbn: "9780525559474",
    tags: ["reflective", "second chances", "bookish"],
    notes: "Jacket and boards look nearly untouched.",
    color: "#204663",
    staffPick: false,
    newArrival: true,
    aspectRatio: 0.77,
    sealStyle: "circle",
    sealText: "SOUL"
  },
  {
    id: "b016",
    title: "The Overstory",
    author: "Richard Powers",
    genre: "Fiction",
    price: 9.75,
    condition: "Good",
    format: "Paperback",
    year: 2019,
    stock: 3,
    isbn: "9780393356687",
    tags: ["trees", "pulitzer", "ambitious"],
    notes: "One cracked spine line, pages bright and unmarked.",
    color: "#2b522b",
    staffPick: true,
    newArrival: false,
    aspectRatio: 0.69,
    sealStyle: "circle",
    sealText: "PUL"
  },
  {
    id: "b017",
    title: "Educated",
    author: "Tara Westover",
    genre: "Memoir",
    price: 7.75,
    condition: "Very Good",
    format: "Paperback",
    year: 2020,
    stock: 4,
    isbn: "9780399590528",
    tags: ["memoir", "resilience", "family"],
    notes: "Light edge wear, no notes or highlighting.",
    color: "#5c3d2e",
    staffPick: false,
    newArrival: false,
    aspectRatio: 0.64,
    sealStyle: "none",
    sealText: ""
  },
  {
    id: "b018",
    title: "The Art Forger",
    author: "B.A. Shapiro",
    genre: "Mystery",
    price: 6.5,
    condition: "Good",
    format: "Trade Paperback",
    year: 2013,
    stock: 2,
    isbn: "9781616203160",
    tags: ["art", "heist", "boston"],
    notes: "Clean copy with a remainder dot.",
    color: "#523b6b",
    staffPick: false,
    newArrival: false,
    aspectRatio: 0.73,
    sealStyle: "diamond",
    sealText: "ART"
  },
  {
    id: "b019",
    title: "Pachinko",
    author: "Min Jin Lee",
    genre: "Historical Fiction",
    price: 10.25,
    condition: "Very Good",
    format: "Paperback",
    year: 2017,
    stock: 3,
    isbn: "9781455563920",
    tags: ["family saga", "korea", "japan"],
    notes: "Minor shelf wear, tight and clean inside.",
    color: "#7d2b24",
    staffPick: true,
    newArrival: true,
    aspectRatio: 0.67,
    sealStyle: "circle",
    sealText: "DB"
  },
  {
    id: "b020",
    title: "The Splendid and the Vile",
    author: "Erik Larson",
    genre: "History",
    price: 12.5,
    condition: "Very Good",
    format: "Hardcover",
    year: 2020,
    stock: 2,
    isbn: "9780385348713",
    tags: ["churchill", "wwii", "narrative"],
    notes: "Jacket has light rubbing, pages pristine.",
    color: "#182d42",
    staffPick: false,
    newArrival: false,
    aspectRatio: 0.79,
    sealStyle: "square",
    sealText: "WAR"
  },
  {
    id: "b021",
    title: "Circe",
    author: "Madeline Miller",
    genre: "Fantasy",
    price: 8.95,
    condition: "Very Good",
    format: "Paperback",
    year: 2019,
    stock: 4,
    isbn: "9780316556323",
    tags: ["mythology", "greek", "literary"],
    notes: "Fresh pages, small crease on back cover.",
    color: "#8c4f1c",
    staffPick: true,
    newArrival: false,
    aspectRatio: 0.66,
    sealStyle: "diamond",
    sealText: "MYTH"
  },
  {
    id: "b022",
    title: "The Body Keeps the Score",
    author: "Bessel van der Kolk",
    genre: "Psychology",
    price: 11.5,
    condition: "Good",
    format: "Paperback",
    year: 2015,
    stock: 2,
    isbn: "9780143127741",
    tags: ["psychology", "trauma", "health"],
    notes: "A few highlighted passages in early chapters.",
    color: "#2d444f",
    staffPick: false,
    newArrival: false,
    aspectRatio: 0.72,
    sealStyle: "none",
    sealText: ""
  },
  {
    id: "b023",
    title: "The Joy of Cooking",
    author: "Irma S. Rombauer",
    genre: "Cookbooks",
    price: 16.95,
    condition: "Good",
    format: "Hardcover",
    year: 2006,
    stock: 1,
    isbn: "9780743246262",
    tags: ["classic", "reference", "cooking"],
    notes: "Sturdy kitchen copy with light page waviness.",
    color: "#6b401d",
    staffPick: false,
    newArrival: false,
    aspectRatio: 0.84,
    sealStyle: "circle",
    sealText: "JOY"
  },
  {
    id: "b024",
    title: "The House in the Cerulean Sea",
    author: "TJ Klune",
    genre: "Fantasy",
    price: 9.95,
    condition: "Like New",
    format: "Paperback",
    year: 2021,
    stock: 3,
    isbn: "9781250217318",
    tags: ["warm", "found family", "hopeful"],
    notes: "Excellent copy with a bright, clean cover.",
    color: "#235c70",
    staffPick: true,
    newArrival: true,
    aspectRatio: 0.69,
    sealStyle: "circle",
    sealText: "LOVE"
  },
  {
    id: "b025",
    title: "Say Nothing",
    author: "Patrick Radden Keefe",
    genre: "History",
    price: 9.5,
    condition: "Very Good",
    format: "Paperback",
    year: 2020,
    stock: 2,
    isbn: "9780307279286",
    tags: ["ireland", "true crime", "politics"],
    notes: "Unmarked pages, faint corner curl.",
    color: "#323528",
    staffPick: false,
    newArrival: false,
    aspectRatio: 0.70,
    sealStyle: "diamond",
    sealText: "TRUE"
  },
  {
    id: "b026",
    title: "The Invisible Life of Addie LaRue",
    author: "V.E. Schwab",
    genre: "Fantasy",
    price: 10.75,
    condition: "Very Good",
    format: "Hardcover",
    year: 2020,
    stock: 2,
    isbn: "9780765387561",
    tags: ["immortality", "romance", "paris"],
    notes: "Jacket lightly rubbed, boards and pages clean.",
    color: "#13273b",
    staffPick: false,
    newArrival: true,
    aspectRatio: 0.78,
    sealStyle: "square",
    sealText: "MAGIC"
  },
  {
    id: "b027",
    title: "Atomic Habits",
    author: "James Clear",
    genre: "Business",
    price: 8.5,
    condition: "Good",
    format: "Paperback",
    year: 2018,
    stock: 5,
    isbn: "9780735211292",
    tags: ["habits", "productivity", "practical"],
    notes: "A few underlined sections, clean cover.",
    color: "#162329",
    staffPick: false,
    newArrival: false,
    aspectRatio: 0.65,
    sealStyle: "circle",
    sealText: "FOCUS"
  },
  {
    id: "b028",
    title: "The Feather Thief",
    author: "Kirk Wallace Johnson",
    genre: "Mystery",
    price: 7.25,
    condition: "Very Good",
    format: "Paperback",
    year: 2019,
    stock: 3,
    isbn: "9781101981634",
    tags: ["true crime", "natural history", "odd"],
    notes: "Clean and square with light shelf scuffs.",
    color: "#2e4a46",
    staffPick: true,
    newArrival: false,
    aspectRatio: 0.71,
    sealStyle: "circle",
    sealText: "FEATH"
  }
];

const conditionRank = {
  "Like New": 4,
  "Very Good": 3,
  Good: 2,
  Fair: 1
};

const state = {
  query: "",
  category: "All",
  genre: "All",
  condition: "All",
  maxPrice: 22,
  formats: new Set(["Hardcover", "Paperback", "Trade Paperback"]),
  stockFilter: "all",
  sort: "featured",
  cart: loadCart()
};

const els = {
  headerSearchInput: document.querySelector("#headerSearchInput"),
  headerSearchForm: document.querySelector("#headerSearchForm"),
  headerGenreSelect: document.querySelector("#headerGenreSelect"),
  heroFeature: document.querySelector("#heroFeature"),
  bestSellerShelf: document.querySelector("#bestSellerShelf"),
  newArrivalShelf: document.querySelector("#newArrivalShelf"),
  dealShelf: document.querySelector("#dealShelf"),
  genreFilter: document.querySelector("#genreFilter"),
  conditionFilter: document.querySelector("#conditionFilter"),
  priceFilter: document.querySelector("#priceFilter"),
  priceValue: document.querySelector("#priceValue"),
  formatInputs: [...document.querySelectorAll("input[name='format']")],
  resetFilters: document.querySelector("#resetFilters"),
  emptyReset: document.querySelector("#emptyReset"),
  sortSelect: document.querySelector("#sortSelect"),
  bookGrid: document.querySelector("#bookGrid"),
  emptyState: document.querySelector("#emptyState"),
  resultSummary: document.querySelector("#resultSummary"),
  miniShelf: document.querySelector("#miniShelf"),
  cartDrawer: document.querySelector("#cartDrawer"),
  cartToggle: document.querySelector(".cart-toggle"),
  closeCart: document.querySelector(".close-cart"),
  cartItems: document.querySelector("#cartItems"),
  cartSubtotal: document.querySelector("#cartSubtotal"),
  cartShipping: document.querySelector("#cartShipping"),
  cartTotal: document.querySelector("#cartTotal"),
  cartCount: document.querySelector("[data-cart-count]"),
  checkoutButton: document.querySelector("#checkoutButton"),
  bookModal: document.querySelector("#bookModal"),
  modalContent: document.querySelector("#modalContent"),
  toast: document.querySelector("#toast"),
  searchSuggestions: document.querySelector("#searchSuggestions"),
  sidebarSearchInput: document.querySelector("#sidebarSearchInput")
};

const money = new Intl.NumberFormat("en-US", {
  style: "currency",
  currency: "USD"
});

function loadCart() {
  try {
    const parsed = JSON.parse(localStorage.getItem("davisBooksCart") || "[]");
    return Array.isArray(parsed) ? parsed : [];
  } catch {
    return [];
  }
}

function saveCart() {
  localStorage.setItem("davisBooksCart", JSON.stringify(state.cart));
}

function escapeHtml(value) {
  return String(value)
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&#039;");
}

function normalize(value) {
  return String(value).trim().toLowerCase();
}

function showSearchHub() {
  const query = els.headerSearchInput.value.trim();
  if (query) {
    showSuggestions(query);
    return;
  }

  const recentSearches = ["Anthony Bourdain", "Classic Novels", "Mystery Thrillers", "ISBN-13"];
  let recentViewed = [];
  try {
    recentViewed = JSON.parse(localStorage.getItem("davisBooksRecent") || "[]");
  } catch (e) {}

  let recentHtml = "";
  if (recentViewed.length > 0) {
    recentHtml = `
      <div class="searchhub-section">
        <div class="searchhub-section-title">Keep shopping for</div>
        <div class="searchhub-recent-grid">
          ${recentViewed.slice(0, 3).map(id => {
            const b = findBook(id);
            if (!b) return "";
            return `
              <div class="searchhub-recent-item" data-action="details" data-book-id="${b.id}">
                <div class="searchhub-mini-cover" style="--cover-color: ${b.color}"></div>
                <div class="searchhub-recent-text">
                  <span class="title">${escapeHtml(b.title)}</span>
                  <span class="price">${money.format(b.price)}</span>
                </div>
              </div>
            `;
          }).join("")}
        </div>
      </div>
    `;
  }

  let html = `
    <div class="search-hub-panel">
      ${recentHtml}
      <div class="searchhub-section">
        <div class="searchhub-section-title">Shop by category</div>
        <div class="searchhub-categories-grid">
          <button class="category-grid-btn" type="button" data-category="Fiction">Fiction</button>
          <button class="category-grid-btn" type="button" data-category="Mystery">Mystery</button>
          <button class="category-grid-btn" type="button" data-category="History">History</button>
          <button class="category-grid-btn" type="button" data-category="Cookbooks">Cookbooks</button>
        </div>
      </div>
      <div class="searchhub-section">
        <div class="searchhub-section-title">Recent searches</div>
        <div class="searchhub-history-list">
          ${recentSearches.map(term => `
            <div class="history-item" data-suggestion-type="history" data-suggestion-value="${escapeHtml(term)}">
              <div class="history-left">
                <span class="history-icon">⏳</span>
                <span class="history-text">${escapeHtml(term)}</span>
              </div>
              <button class="history-remove" type="button" aria-label="Remove search history">×</button>
            </div>
          `).join("")}
        </div>
      </div>
    </div>
  `;

  els.searchSuggestions.innerHTML = html;
  els.searchSuggestions.hidden = false;
}

function showSuggestions(query) {
  const term = normalize(query);
  if (!term) {
    els.searchSuggestions.hidden = true;
    return;
  }

  const matchedGenres = [...new Set(books.map((b) => b.genre))]
    .filter((g) => normalize(g).includes(term))
    .slice(0, 3);

  const matchedBooks = books
    .filter((b) => normalize(b.title).includes(term) || normalize(b.author).includes(term) || normalize(b.isbn).includes(term))
    .slice(0, 5);

  if (matchedGenres.length === 0 && matchedBooks.length === 0) {
    els.searchSuggestions.hidden = true;
    return;
  }

  let html = "";
  if (matchedGenres.length > 0) {
    html += `<div class="suggestion-group-title">Genres</div>`;
    html += matchedGenres
      .map(
        (genre) => `
        <div class="suggestion-item" data-suggestion-type="genre" data-suggestion-value="${escapeHtml(genre)}">
          <span class="suggestion-title">${escapeHtml(genre)}</span>
          <span class="suggestion-meta">Genre</span>
        </div>
      `
      )
      .join("");
  }

  if (matchedBooks.length > 0) {
    html += `<div class="suggestion-group-title">Books & Authors</div>`;
    html += matchedBooks
      .map(
        (book) => `
        <div class="suggestion-item" data-suggestion-type="book" data-suggestion-value="${escapeHtml(book.title)}">
          <div>
            <span class="suggestion-title">${escapeHtml(book.title)}</span>
            <span class="suggestion-author">by ${escapeHtml(book.author)}</span>
          </div>
          <span class="suggestion-meta">${money.format(book.price)}</span>
        </div>
      `
      )
      .join("");
  }

  els.searchSuggestions.innerHTML = html;
  els.searchSuggestions.hidden = false;
}

function populateGenreFilter() {
  const genres = [...new Set(books.map((book) => book.genre))].sort();
  const optionsHtml = genres.map((genre) => `<option value="${escapeHtml(genre)}">${escapeHtml(genre)}</option>`).join("");
  els.genreFilter.insertAdjacentHTML("beforeend", optionsHtml);
  if (els.headerGenreSelect) {
    els.headerGenreSelect.insertAdjacentHTML("beforeend", optionsHtml);
  }
}

function getFilteredBooks() {
  const query = normalize(state.query);
  const filtered = books.filter((book) => {
    const haystack = normalize(
      [book.title, book.author, book.genre, book.isbn, ...book.tags].join(" ")
    );
    const categoryMatch = state.category === "All" || book.genre === state.category;
    const genreMatch = state.genre === "All" || book.genre === state.genre;
    const conditionMatch = state.condition === "All" || book.condition === state.condition;
    const priceMatch = book.price <= state.maxPrice;
    const formatMatch = state.formats.has(book.format);
    const queryMatch = query === "" || haystack.includes(query);
    const stockMatch =
      state.stockFilter === "all" ||
      (state.stockFilter === "staff" && book.staffPick) ||
      (state.stockFilter === "new" && book.newArrival) ||
      (state.stockFilter === "deal" && book.price < 8);

    return (
      queryMatch &&
      categoryMatch &&
      genreMatch &&
      conditionMatch &&
      priceMatch &&
      formatMatch &&
      stockMatch
    );
  });

  return filtered.sort(sortBooks);
}

function sortBooks(a, b) {
  if (state.sort === "title") {
    return a.title.localeCompare(b.title);
  }
  if (state.sort === "priceLow") {
    return a.price - b.price;
  }
  if (state.sort === "priceHigh") {
    return b.price - a.price;
  }
  if (state.sort === "condition") {
    return conditionRank[b.condition] - conditionRank[a.condition] || a.price - b.price;
  }

  return Number(b.staffPick) - Number(a.staffPick) || Number(b.newArrival) - Number(a.newArrival);
}

function renderBooks() {
  const filtered = getFilteredBooks();
  els.resultSummary.textContent = `${filtered.length} of ${books.length} used books shown`;
  els.emptyState.hidden = filtered.length > 0;
  els.bookGrid.hidden = filtered.length === 0;

  els.bookGrid.innerHTML = filtered.map(renderBookCard).join("");
}

function renderUniversalBookCard(book, isCatalog = false) {
  // Free store pickup tomorrow is available for books over $8.0
  const isFreePickup = book.price > 8.0;
  
  return `
    <article class="product-tile ${isCatalog ? 'book-card' : ''}" data-action="details" data-book-id="${book.id}" style="--cover-color: ${book.color}">
      ${renderCover(book)}
      <div class="product-copy">
        <strong>${escapeHtml(book.title)}</strong>
        <span class="author">by ${escapeHtml(book.author)}</span>
        <div class="stars-rating">
          <span class="stars-val">★★★★★</span>
          <span class="rating-count">${isCatalog ? '124' : '48'}</span>
        </div>
        <div class="price-row">
          <span class="price"><span class="price-prefix">from </span><strong class="price-val">${money.format(book.price)}</strong></span>
        </div>
        <div class="condition-row">
          <span class="pill condition">${escapeHtml(book.condition)}</span>
        </div>
      </div>
      <div class="product-actions-wrapper">
        ${isFreePickup ? `<div class="shipping-tag">FREE store pickup tomorrow</div>` : ''}
        <div class="card-double-buttons">
          <button class="card-btn add-btn" type="button" data-action="add" data-book-id="${book.id}">Add to Cart</button>
          <button class="card-btn buy-now-btn" type="button" data-action="buy-now-card" data-book-id="${book.id}">Buy Now</button>
        </div>
      </div>
    </article>
  `;
}

function renderBookCard(book) {
  return renderUniversalBookCard(book, true);
}

function renderProductTile(book) {
  return renderUniversalBookCard(book, false);
}

function renderStorefront() {
  const bestSellers = ["b005", "b003", "b024", "b004", "b009", "b019"]
    .map(findBook)
    .filter(Boolean);
  const newArrivals = books.filter((book) => book.newArrival).slice(0, 6);
  const deals = books.filter((book) => book.price < 8).slice(0, 6);

  const featured = findBook("b005");
  const singleFeatureEl = document.getElementById("heroSingleFeature");
  if (featured && singleFeatureEl) {
    const discountPercent = Math.round((1 - (featured.price / (featured.price * 1.5))) * 100);
    singleFeatureEl.innerHTML = `
      <div class="feature-tile" data-action="details" data-book-id="${featured.id}">
        <div class="feature-tile-cover">
          ${renderCover(featured)}
        </div>
        <div class="feature-tile-info">
          <h4>${escapeHtml(featured.title)}</h4>
          <p class="author">by ${escapeHtml(featured.author)}</p>
          <div class="stars-rating">
            <span class="stars-val">★★★★★</span>
            <span class="rating-count">85</span>
          </div>
          <div class="price-row">
            <span class="discount-badge">-${discountPercent}%</span>
            <span class="price"><span class="price-prefix">from </span><strong class="price-val">${money.format(featured.price)}</strong></span>
          </div>
          <div class="condition-row" style="margin-top: 4px;">
            <span class="pill condition">${escapeHtml(featured.condition)}</span>
          </div>
          <div class="shipping-tag" style="margin-top: 4px;">
            FREE store pickup tomorrow
          </div>
          <div class="card-double-buttons" style="margin-top: 10px;">
            <button class="card-btn add-btn" type="button" data-action="add" data-book-id="${featured.id}">Add to Cart</button>
            <button class="card-btn buy-now-btn" type="button" data-action="buy-now-card" data-book-id="${featured.id}">Buy Now</button>
          </div>
        </div>
      </div>
    `;
  }

  els.bestSellerShelf.innerHTML = bestSellers.map(renderProductTile).join("");
  els.newArrivalShelf.innerHTML = newArrivals.map(renderProductTile).join("");
  els.dealShelf.innerHTML = deals.map(renderProductTile).join("");

  setTimeout(() => {
    document.querySelectorAll(".product-row").forEach((row) => {
      if (typeof updateCarouselButtons === "function") {
        updateCarouselButtons(row);
      }
    });
  }, 150);
}

function renderCover(book) {
  const aspect = book.aspectRatio || 0.68;
  return `
    <div class="book-cover-container" style="--book-aspect-ratio: ${aspect}; --cover-color: ${book.color}">
      <div class="book-cover" aria-hidden="true">
        <div class="cover-text">
          <span class="cover-title">${escapeHtml(book.title)}</span>
          <span class="cover-author">${escapeHtml(book.author)}</span>
        </div>
      </div>
    </div>
  `;
}

function renderMiniShelf() {
  const picks = books.filter((book) => book.staffPick).slice(0, 3);
  
  const staffQuotes = {
    "b001": `"An atmospheric, dream-like escape. It feels like stepping into a midnight festival." — Sarah`,
    "b003": `"The definitive epic. The worldbuilding is so rich you'll feel the sand on your fingers." — Dan`,
    "b004": `"A dark, campus mystery that is impossible to put down. Truly haunting." — Claire`
  };

  els.miniShelf.innerHTML = picks
    .map(
      (book) => {
        const quote = staffQuotes[book.id] || `"A wonderful pre-loved copy, carefully chosen for our shelf." — Davis Team`;
        return `
          <article class="staff-pick-card" data-action="details" data-book-id="${book.id}">
            <div class="staff-pick-cover">
              ${renderCover(book)}
            </div>
            <div class="staff-pick-info">
              <span class="staff-badge">★ Staff Pick</span>
              <h4>${escapeHtml(book.title)}</h4>
              <span class="author">by ${escapeHtml(book.author)}</span>
              <p class="staff-quote">${escapeHtml(quote)}</p>
              <div class="staff-footer">
                <span class="price">from <strong>${money.format(book.price)}</strong></span>
                <button class="staff-btn" type="button">Details</button>
              </div>
            </div>
          </article>
        `;
      }
    )
    .join("");
}

function findBook(id) {
  return books.find((book) => book.id === id);
}

function getDeterministicCopiesForBook(book) {
  const idNum = book.id.charCodeAt(3) || 0;
  const conditions = [
    { name: "New (Sealed)", multiplier: 2.2, seed: 1 },
    { name: "New (Open)", multiplier: 2.0, seed: 2 },
    { name: "Fine", multiplier: 1.6, seed: 3 },
    { name: "Very Good", multiplier: 1.3, seed: 4 },
    { name: "Good", multiplier: 1.0, seed: 5 },
    { name: "Fair", multiplier: 0.75, seed: 6 },
    { name: "Poor", multiplier: 0.5, seed: 7 }
  ];

  const bookCondMult = conditions.find(c => c.name === book.condition)?.multiplier || 1.0;
  const basePrice = book.price / bookCondMult;

  return conditions.map(c => {
    if (c.name === book.condition) {
      return {
        condition: c.name,
        price: book.price,
        stock: book.stock
      };
    } else {
      const stock = (idNum + c.seed) % 3;
      let price = basePrice * c.multiplier;
      price = Math.round(price * 4) / 4;
      if (price < 3.95) price = 3.95;
      return {
        condition: c.name,
        price: price,
        stock: stock
      };
    }
  }).filter(c => c.stock > 0);
}

function getStockForCopy(book, condition) {
  const copies = getDeterministicCopiesForBook(book);
  const copy = copies.find((c) => c.condition === condition);
  return copy ? copy.stock : 0;
}

function addToCart(id, condition = null, price = null) {
  const book = findBook(id);
  if (!book) return;

  const targetCondition = condition || book.condition;
  const targetPrice = price !== null ? parseFloat(price) : book.price;

  const item = state.cart.find(
    (entry) => entry.id === id && entry.condition === targetCondition
  );

  const maxStock = getStockForCopy(book, targetCondition);

  if (item) {
    if (item.quantity >= maxStock) {
      showToast(`Only ${maxStock} copies of ${book.title} (${targetCondition}) are in stock.`);
      return;
    }
    item.quantity += 1;
  } else {
    state.cart.push({
      id,
      condition: targetCondition,
      price: targetPrice,
      quantity: 1
    });
  }

  saveCart();
  renderCart();
  showToast(`${book.title} (${targetCondition}) added to cart.`);
}

function updateQuantity(id, delta, condition) {
  const book = findBook(id);
  const item = state.cart.find((entry) => entry.id === id && entry.condition === condition);
  if (!book || !item) return;

  const maxStock = getStockForCopy(book, condition);

  item.quantity += delta;
  if (item.quantity <= 0) {
    removeFromCart(id, condition);
    return;
  }
  if (item.quantity > maxStock) {
    item.quantity = maxStock;
    showToast(`Only ${maxStock} copies available.`);
  }

  saveCart();
  renderCart();
}

function removeFromCart(id, condition) {
  state.cart = state.cart.filter((entry) => !(entry.id === id && entry.condition === condition));
  saveCart();
  renderCart();
}

function getCartLines() {
  return state.cart
    .map((entry) => {
      const book = findBook(entry.id);
      return book ? { ...entry, book } : null;
    })
    .filter(Boolean);
}

function renderCart() {
  const lines = getCartLines();
  const subtotal = lines.reduce((sum, entry) => sum + entry.price * entry.quantity, 0);
  const shipping = subtotal === 0 || subtotal >= 35 ? 0 : 4.95;
  const total = subtotal + shipping;
  const itemCount = lines.reduce((sum, entry) => sum + entry.quantity, 0);

  els.cartCount.textContent = String(itemCount);
  els.cartSubtotal.textContent = money.format(subtotal);
  els.cartShipping.textContent = shipping === 0 ? "Free" : money.format(shipping);
  els.cartTotal.textContent = money.format(total);
  els.checkoutButton.disabled = lines.length === 0;

  const progressText = document.getElementById("shippingProgressText");
  const progressFill = document.getElementById("shippingProgressFill");
  if (progressText && progressFill) {
    if (subtotal === 0) {
      progressText.textContent = "Your cart is empty. Add $35.00 more for Free Shipping!";
      progressFill.style.width = "0%";
    } else if (subtotal >= 35) {
      progressText.innerHTML = "🎉 Congratulations! Your order qualifies for <strong>FREE SHIPPING</strong>!";
      progressFill.style.width = "100%";
    } else {
      const needed = (35 - subtotal).toFixed(2);
      progressText.innerHTML = `Add <strong>$${needed}</strong> more for <strong>FREE SHIPPING</strong>!`;
      const pct = Math.min((subtotal / 35) * 100, 100);
      progressFill.style.width = `${pct}%`;
    }
  }

  // Update homepage Stack Builder widget
  const overlayText = document.getElementById("overlayProgressText");
  const overlayFill = document.getElementById("overlayProgressBar");
  if (overlayText && overlayFill) {
    if (subtotal === 0) {
      overlayText.innerHTML = "Add books to start your stack!";
      overlayFill.style.width = "0%";
    } else if (subtotal >= 35) {
      overlayText.innerHTML = "🎉 Your stack qualifies for <strong>FREE SHIPPING</strong>!";
      overlayFill.style.width = "100%";
    } else {
      const needed = (35 - subtotal).toFixed(2);
      overlayText.innerHTML = `Add <strong>$${needed}</strong> more for <strong>FREE SHIPPING</strong>!`;
      const pct = Math.min((subtotal / 35) * 100, 100);
      overlayFill.style.width = `${pct}%`;
    }
  }

  const fillersEl = document.getElementById("overlayQuickFillers");
  if (fillersEl) {
    const cheapBooks = books.filter(b => b.price < 6.5).slice(0, 2);
    fillersEl.innerHTML = cheapBooks.map(b => `
      <div class="filler-book-row" data-action="details" data-book-id="${b.id}">
        <div class="filler-cover-mini" style="--cover-color: ${b.color}"></div>
        <div class="filler-details">
          <span class="filler-title">${escapeHtml(b.title)}</span>
          <span class="filler-price">${money.format(b.price)}</span>
          <button class="filler-add-btn" type="button" data-action="add" data-book-id="${b.id}">+ Add</button>
        </div>
      </div>
    `).join("");
  }

  if (lines.length === 0) {
    els.cartItems.innerHTML = `
      <div class="cart-empty">
        <strong>Your cart is empty</strong>
        <span>Fresh finds are waiting in the catalog.</span>
      </div>
    `;
    return;
  }

  els.cartItems.innerHTML = lines
    .map(
      (entry) => `
        <div class="cart-item" style="--cover-color: ${entry.book.color}">
          <div class="cart-cover" aria-hidden="true"></div>
          <div class="cart-item-main">
            <div class="cart-title-row">
              <strong>${escapeHtml(entry.book.title)}</strong>
              <span>${money.format(entry.price * entry.quantity)}</span>
            </div>
            <p class="cart-author">by ${escapeHtml(entry.book.author)}</p>
            <p class="cart-item-condition" style="font-size: 0.8rem; color: var(--muted); margin: 2px 0 6px;">
              ${escapeHtml(entry.condition)} · ${money.format(entry.price)} each
            </p>
            <div class="quantity-row">
              <div class="stepper" aria-label="Quantity for ${escapeHtml(entry.book.title)} (${escapeHtml(entry.condition)})">
                <button type="button" data-action="decrease" data-book-id="${entry.id}" data-condition="${escapeHtml(entry.condition)}" aria-label="Decrease quantity">-</button>
                <span>${entry.quantity}</span>
                <button type="button" data-action="increase" data-book-id="${entry.id}" data-condition="${escapeHtml(entry.condition)}" aria-label="Increase quantity">+</button>
              </div>
              <button class="remove-button" type="button" data-action="remove" data-book-id="${entry.id}" data-condition="${escapeHtml(entry.condition)}">Remove</button>
            </div>
          </div>
        </div>
      `
    )
    .join("");
}

function openCart() {
  els.cartDrawer.classList.add("is-open");
  els.cartDrawer.setAttribute("aria-hidden", "false");
  els.cartToggle.setAttribute("aria-expanded", "true");
  document.body.classList.add("cart-open");
  els.closeCart.focus();
}

function closeCart() {
  els.cartDrawer.classList.remove("is-open");
  els.cartDrawer.setAttribute("aria-hidden", "true");
  els.cartToggle.setAttribute("aria-expanded", "false");
  document.body.classList.remove("cart-open");
  els.cartToggle.focus();
}

function openBookModal(id) {
  const book = findBook(id);
  if (!book) return;

  els.modalContent.innerHTML = `
    ${renderCover(book)}
    <div class="modal-details">
      <button class="icon-only" type="button" data-action="close-modal" aria-label="Close details">
        <svg viewBox="0 0 24 24" aria-hidden="true" focusable="false">
          <path d="M6 6l12 12M18 6 6 18"></path>
        </svg>
      </button>
      <div>
        <p class="eyebrow">${escapeHtml(book.genre)}</p>
        <h2 style="font-family: var(--font-serif); font-size: clamp(1.5rem, 2.5vw, 2.1rem); line-height: 1.2;">${escapeHtml(book.title)}</h2>
        <p class="book-author" style="margin-top: 4px;">by ${escapeHtml(book.author)}</p>
      </div>
      
      <!-- Modal Tabs Header -->
      <div class="modal-tabs">
        <button class="modal-tab-button is-active" type="button" data-tab-target="tab-about">About</button>
        <button class="modal-tab-button" type="button" data-tab-target="tab-specs">Specs</button>
        <button class="modal-tab-button" type="button" data-tab-target="tab-condition">Condition Guide</button>
      </div>

      <!-- Tab Contents -->
      <div class="modal-tab-content is-active" id="tab-about">
        <p>${escapeHtml(book.notes)}</p>
        <p class="book-notes" style="margin-top: 10px; color: var(--rust); font-weight: 600;">
          ${book.tags.map((tag) => escapeHtml(`#${tag}`)).join(" ")}
        </p>
      </div>

      <div class="modal-tab-content" id="tab-specs">
        <table class="specs-table">
          <tr><td>ISBN</td><td>${escapeHtml(book.isbn)}</td></tr>
          <tr><td>Year Published</td><td>${book.year}</td></tr>
          <tr><td>Format</td><td>${escapeHtml(book.format)}</td></tr>
          <tr><td>Condition</td><td>${escapeHtml(book.condition)}</td></tr>
        </table>
      </div>

      <div class="modal-tab-content" id="tab-condition">
        <p style="margin-bottom: 8px;"><strong>Grading Standard: ${escapeHtml(book.condition)}</strong></p>
        <p style="font-size: 0.88rem; color: var(--muted);">
          ${book.condition === 'Like New' ? 'This copy is in pristine condition. Spine is uncreased, cover is completely unmarked, and pages are bright white.' : ''}
          ${book.condition === 'Very Good' ? 'Minimal signs of shelf wear. Spine is flat and tight, pages are clean without highlights or marks, minor corner wear.' : ''}
          ${book.condition === 'Good' ? 'Average used copy. May have minor reading creases on the spine, light tanning on page edges, or small annotations.' : ''}
          ${book.condition === 'Fair' ? 'A well-loved reading copy. Binding is solid but cover and pages show wear. May have creases, library stamps, or annotations.' : ''}
        </p>
      </div>

      <div class="price-line" style="margin-top: 12px; display: flex; justify-content: space-between; align-items: center;">
        <span class="price" style="font-size: 1.4rem; color: var(--sage);">${money.format(book.price)}</span>
        <span class="stock" style="font-size: 0.88rem; color: var(--muted);">${book.stock} copies in stock</span>
      </div>
      
      <div class="modal-actions">
        <button class="primary-button" type="button" data-action="add" data-book-id="${book.id}">Add to cart</button>
        <button class="secondary-button" type="button" data-action="close-modal">Keep browsing</button>
      </div>
    </div>
  `;

  els.bookModal.showModal();
}

function showBookDetails(id) {
  const book = findBook(id);
  if (!book) return;

  // Save to recently viewed
  let recent = [];
  try {
    recent = JSON.parse(localStorage.getItem("davisBooksRecent") || "[]");
  } catch (e) {}
  recent = [id, ...recent.filter(item => item !== id)].slice(0, 5);
  localStorage.setItem("davisBooksRecent", JSON.stringify(recent));

  // 1. Hide storefront view, show book details view
  document.getElementById("storefrontView").hidden = true;
  const detailsView = document.getElementById("bookDetailsView");
  detailsView.hidden = false;

  // 2. Generate details page HTML
  const copies = getDeterministicCopiesForBook(book);
  const copiesHtml = copies.map(c => `
    <tr>
      <td>${c.stock}</td>
      <td><strong>${escapeHtml(c.condition)}</strong></td>
      <td class="price"><strong>${money.format(c.price)}</strong></td>
      <td>
        <button class="small-action add" type="button" data-action="add-copy" data-book-id="${book.id}" data-condition="${escapeHtml(c.condition)}" data-price="${c.price}">
          Add to stack
        </button>
      </td>
    </tr>
  `).join("");

  const discountPercent = Math.round((1 - (book.price / (book.price * 1.5))) * 100);

  detailsView.innerHTML = `
    <div class="details-nav">
      <button class="back-link" type="button" data-action="back-to-storefront">
        <svg class="back-icon" viewBox="0 0 24 24" aria-hidden="true" focusable="false">
          <path d="M19 12H5M12 19l-7-7 7-7"/>
        </svg>
        Back to browse
      </button>
    </div>
    
    <div class="details-grid-3col">
      <!-- COLUMN 1: Visuals & Thumbnails -->
      <div class="details-left-media">
        <div class="thumbnail-strip">
          <button class="thumb-btn is-active" type="button" aria-label="Cover front">
            <span class="thumb-color-block front" style="background-color: ${book.color}"></span>
          </button>
          <button class="thumb-btn" type="button" aria-label="Cover spine crease">
            <span class="thumb-color-block spine" style="background-color: ${book.color}"></span>
          </button>
          <button class="thumb-btn" type="button" aria-label="Cover back page">
            <span class="thumb-color-block back" style="background-color: ${book.color}"></span>
          </button>
        </div>
        <div class="main-cover-wrapper">
          ${renderCover(book)}
        </div>
      </div>

      <!-- COLUMN 2: Product Specifications & Copy -->
      <div class="details-middle-specs">
        <p class="details-brand-link">Visit the <a href="#catalog" data-action="back-to-storefront">Visit the ${escapeHtml(book.author)} Store</a></p>
        
        <h1 class="details-title">${escapeHtml(book.title)}</h1>
        <p class="details-author">by <span class="author-name">${escapeHtml(book.author)}</span></p>
        
        <div class="details-rating">
          <div class="stars">★★★★★</div>
          <span class="rating-text">4.8 (124 ratings on Davis's)</span>
        </div>

        <div class="details-price-row">
          <span class="discount-badge">-${discountPercent}%</span>
          <span class="price-symbol">$</span>
          <span class="price-val-large">${Math.floor(book.price)}</span>
          <span class="price-cents-large">${Math.round((book.price % 1) * 100).toString().padStart(2, '0')}</span>
        </div>
        
        <div class="list-price-row">
          List Price: <span class="list-price-strike">${money.format(book.price * 1.5)}</span>
        </div>

        <div class="details-quick-specs">
          <span><strong>Published:</strong> ${book.year}</span>
          <span><strong>Format:</strong> ${escapeHtml(book.format)}</span>
          <span><strong>ISBN:</strong> ${escapeHtml(book.isbn)}</span>
        </div>

        <div class="details-description">
          <h2>About this copy</h2>
          <p>${escapeHtml(book.notes || "Clean readable pages, sturdy binding.")}</p>
        </div>

        <div class="details-copies-section">
          <h2>Other Sellers on Davis's</h2>
          <table class="copies-table">
            <thead>
              <tr>
                <th>Stock</th>
                <th>Condition</th>
                <th>Price</th>
                <th></th>
              </tr>
            </thead>
            <tbody>
              ${copiesHtml}
            </tbody>
          </table>
        </div>
        
        <div class="details-tabs-section">
          <div class="modal-tabs">
            <button class="modal-tab-button is-active" type="button" data-tab-target="details-tab-specs">Specifications</button>
            <button class="modal-tab-button" type="button" data-tab-target="details-tab-guide">Condition Guide</button>
          </div>
          <div class="modal-tab-content is-active" id="details-tab-specs">
            <table class="specs-table">
              <tbody>
                <tr><td>Publisher</td><td>Davis's Press</td></tr>
                <tr><td>ISBN-10</td><td>${escapeHtml(book.isbn.substring(3))}</td></tr>
                <tr><td>ISBN-13</td><td>${escapeHtml(book.isbn)}</td></tr>
                <tr><td>Language</td><td>English</td></tr>
                <tr><td>Pages</td><td>${book.id.charCodeAt(1) * 3 + 240} pages</td></tr>
              </tbody>
            </table>
          </div>
          <div class="modal-tab-content" id="details-tab-guide">
            <p><strong>New (Sealed):</strong> Brand new copy in original shrink wrap.</p>
            <p><strong>New (Open):</strong> Brand new copy, unread but shrink wrap removed.</p>
            <p><strong>Fine:</strong> Crisp, looks unread, near perfect condition.</p>
            <p><strong>Very Good:</strong> Small shelf wear, tight binding, clean text.</p>
            <p><strong>Good:</strong> Standard readable copy, minor markings or creasing.</p>
            <p><strong>Fair:</strong> Heavy wear, complete text, ideal reading copy.</p>
            <p><strong>Poor:</strong> Worn, but fully intact and readable.</p>
          </div>
        </div>
      </div>

      <!-- COLUMN 3: Sticky Buy Box Card -->
      <div class="details-right-buybox">
        <div class="buybox-card">
          <div class="buybox-price-line">
            <span class="buybox-price" id="buyBoxPrice">${money.format(book.price)}</span>
            <span class="prime-badge-icon">✓ prime</span>
          </div>

          <div class="buybox-delivery">
            <span class="delivery-highlight">FREE delivery</span> on orders over $35.
            <span class="delivery-time">Arrives: <strong>Wednesday, June 30</strong></span>
          </div>

          <div class="buybox-stock-status" id="buyBoxStock">
            <span class="in-stock">In Stock.</span>
          </div>

          <div class="buybox-condition-wrapper">
            <label for="buyBoxConditionSelect">Select copy condition:</label>
            <select id="buyBoxConditionSelect" class="buybox-select">
              ${copies.map(c => `
                <option value="${escapeHtml(c.condition)}" data-price="${c.price}" data-stock="${c.stock}">
                  ${escapeHtml(c.condition)} — ${money.format(c.price)}
                </option>
              `).join("")}
            </select>
          </div>

          <div class="buybox-actions">
            <button class="buybox-btn add-to-stack" id="buyBoxAddBtn" type="button" data-action="add-copy" data-book-id="${book.id}">
              Add to Stack
            </button>
            <button class="buybox-btn buy-now" id="buyBoxBuyNowBtn" type="button">
              Buy Now
            </button>
          </div>

          <div class="buybox-merchant-details">
            <div class="merchant-row">
              <span class="merchant-label">Ships from</span>
              <span class="merchant-value">Davis's Books</span>
            </div>
            <div class="merchant-row">
              <span class="merchant-label">Sold by</span>
              <span class="merchant-value">Davis's Books</span>
            </div>
          </div>
        </div>
      </div>
    </div>
  `;

  // Bind the interactive Buy Box condition change handler
  const condSelect = detailsView.querySelector("#buyBoxConditionSelect");
  if (condSelect) {
    const updateBuyBox = () => {
      const opt = condSelect.options[condSelect.selectedIndex];
      const price = parseFloat(opt.dataset.price);
      const stock = parseInt(opt.dataset.stock, 10);
      const condition = opt.value;

      detailsView.querySelector("#buyBoxPrice").textContent = money.format(price);
      
      const stockEl = detailsView.querySelector("#buyBoxStock");
      if (stock === 1) {
        stockEl.innerHTML = `<span class="low-stock">Only 1 copy left - order soon.</span>`;
      } else {
        stockEl.innerHTML = `<span class="in-stock">In Stock.</span>`;
      }

      // Update the dataset on the add-to-cart button
      const addBtn = detailsView.querySelector("#buyBoxAddBtn");
      addBtn.dataset.condition = condition;
      addBtn.dataset.price = price;
    };

    condSelect.addEventListener("change", updateBuyBox);
    // Trigger initial update
    updateBuyBox();
  }

  // Bind click listener for thumbnails inside the details page
  const thumbButtons = detailsView.querySelectorAll(".thumb-btn");
  thumbButtons.forEach((btn, idx) => {
    btn.addEventListener("click", () => {
      thumbButtons.forEach(b => b.classList.remove("is-active"));
      btn.classList.add("is-active");
      
      // Simulate changing cover look (e.g. angle tilt or spin visual crease highlights)
      const bookCover = detailsView.querySelector(".book-cover");
      if (bookCover) {
        if (idx === 1) {
          bookCover.style.filter = "contrast(1.05) brightness(0.95)";
          bookCover.style.transform = "rotateY(-10deg)";
        } else if (idx === 2) {
          bookCover.style.filter = "contrast(0.9) sepia(0.1)";
          bookCover.style.transform = "rotateY(10deg)";
        } else {
          bookCover.style.filter = "none";
          bookCover.style.transform = "none";
        }
      }
    });
  });

  // Scroll to top of details page
  window.scrollTo({ top: 0, behavior: "smooth" });
}

function showToast(message) {
  els.toast.textContent = message;
  els.toast.classList.add("is-visible");
  clearTimeout(showToast.timeout);
  showToast.timeout = setTimeout(() => {
    els.toast.classList.remove("is-visible");
  }, 2600);
}

function resetFilters() {
  state.query = "";
  state.category = "All";
  state.genre = "All";
  state.condition = "All";
  state.maxPrice = 22;
  state.formats = new Set(["Hardcover", "Paperback", "Trade Paperback"]);
  state.stockFilter = "all";
  state.sort = "featured";

  els.headerSearchInput.value = "";
  els.headerGenreSelect.value = "All";
  els.sidebarSearchInput.value = "";
  els.genreFilter.value = "All";
  els.conditionFilter.value = "All";
  els.priceFilter.value = "22";
  els.priceValue.textContent = "$22";
  els.sortSelect.value = "featured";
  els.formatInputs.forEach((input) => {
    input.checked = true;
  });
  document.querySelectorAll("[data-category]").forEach((button) => {
    button.classList.toggle("is-active", button.dataset.category === "All");
  });
  document.querySelectorAll("[data-stock-filter]").forEach((button) => {
    button.classList.toggle("is-active", button.dataset.stockFilter === "all");
  });

  renderBooks();
}

function updateCarouselButtons(shelf) {
  const shelfId = shelf.id;
  const prevBtn = document.querySelector(`[data-carousel-prev="${shelfId}"]`);
  const nextBtn = document.querySelector(`[data-carousel-next="${shelfId}"]`);
  if (!prevBtn || !nextBtn) return;

  const isAtStart = shelf.scrollLeft <= 5;
  const isAtEnd = shelf.scrollLeft + shelf.clientWidth >= shelf.scrollWidth - 5;

  prevBtn.disabled = isAtStart;
  prevBtn.style.opacity = isAtStart ? "0.35" : "1";
  prevBtn.style.pointerEvents = isAtStart ? "none" : "auto";

  nextBtn.disabled = isAtEnd;
  nextBtn.style.opacity = isAtEnd ? "0.35" : "1";
  nextBtn.style.pointerEvents = isAtEnd ? "none" : "auto";
}

function wireEvents() {
  els.headerSearchForm.addEventListener("submit", (event) => {
    event.preventDefault();
    state.query = els.headerSearchInput.value;
    document.querySelector("#catalog").scrollIntoView({ behavior: "smooth", block: "start" });
    renderBooks();
  });

  els.headerSearchInput.addEventListener("focus", showSearchHub);
  els.headerSearchInput.addEventListener("click", showSearchHub);

  els.headerSearchInput.addEventListener("input", () => {
    state.query = els.headerSearchInput.value;
    els.sidebarSearchInput.value = state.query;
    showSuggestions(state.query);
    renderBooks();
  });

  els.headerGenreSelect.addEventListener("change", () => {
    state.genre = els.headerGenreSelect.value;
    els.genreFilter.value = state.genre;
    renderBooks();
  });

  els.sidebarSearchInput.addEventListener("input", () => {
    state.query = els.sidebarSearchInput.value;
    els.headerSearchInput.value = state.query;
    renderBooks();
  });

  document.querySelectorAll("[data-category]").forEach((button) => {
    button.addEventListener("click", () => {
      state.category = button.dataset.category;
      document.querySelectorAll("[data-category]").forEach((item) => {
        item.classList.toggle("is-active", item === button);
      });
      renderBooks();
    });
  });

  document.querySelectorAll("[data-stock-filter]").forEach((button) => {
    button.addEventListener("click", () => {
      state.stockFilter = button.dataset.stockFilter;
      document.querySelectorAll("[data-stock-filter]").forEach((item) => {
        item.classList.toggle("is-active", item === button);
      });
      renderBooks();
    });
  });

  document.querySelectorAll("[data-shelf-filter]").forEach((button) => {
    button.addEventListener("click", () => {
      resetFilters();
      state.stockFilter = button.dataset.shelfFilter;
      document.querySelectorAll("[data-stock-filter]").forEach((item) => {
        item.classList.toggle("is-active", item.dataset.stockFilter === state.stockFilter);
      });
      renderBooks();
      document.querySelector("#catalog").scrollIntoView({ behavior: "smooth", block: "start" });
    });
  });

  els.genreFilter.addEventListener("change", () => {
    state.genre = els.genreFilter.value;
    renderBooks();
  });

  els.conditionFilter.addEventListener("change", () => {
    state.condition = els.conditionFilter.value;
    renderBooks();
  });

  els.priceFilter.addEventListener("input", () => {
    state.maxPrice = Number(els.priceFilter.value);
    els.priceValue.textContent = `$${state.maxPrice}`;
    renderBooks();
  });

  els.formatInputs.forEach((input) => {
    input.addEventListener("change", () => {
      state.formats = new Set(
        els.formatInputs.filter((formatInput) => formatInput.checked).map((formatInput) => formatInput.value)
      );
      renderBooks();
    });
  });

  els.sortSelect.addEventListener("change", () => {
    state.sort = els.sortSelect.value;
    renderBooks();
  });

  els.resetFilters.addEventListener("click", resetFilters);
  els.emptyReset.addEventListener("click", resetFilters);

  document.addEventListener("click", (event) => {
    // 1. Intercept "Add to cart" button click specifically (since it is inside the clickable card)
    const addBtn = event.target.closest('[data-action="add"]');
    if (addBtn) {
      addToCart(addBtn.dataset.bookId);
      return;
    }

    // 1aa. Intercept Buy Now buttons on product cards
    const buyNowCardBtn = event.target.closest('[data-action="buy-now-card"]');
    if (buyNowCardBtn) {
      addToCart(buyNowCardBtn.dataset.bookId);
      openCart();
      return;
    }

    // 1ab. Intercept price filter buttons from overlay grid
    const priceFilterBtn = event.target.closest('[data-price-filter]');
    if (priceFilterBtn) {
      const val = Number(priceFilterBtn.dataset.priceFilter);
      state.maxPrice = val;
      els.priceFilter.value = String(val);
      els.priceValue.textContent = `$${val}`;
      renderBooks();
      document.querySelector("#catalog").scrollIntoView({ behavior: "smooth", block: "start" });
      return;
    }

    // 1b. Intercept header navigation links & logo to return to storefront view
    const navLink = event.target.closest(".primary-nav a, .subnav-links a, .site-logo");
    if (navLink) {
      document.getElementById("storefrontView").hidden = false;
      document.getElementById("bookDetailsView").hidden = true;
    }

    // 2. Intercept quantity and remove clicks inside the cart drawer
    const increaseBtn = event.target.closest('[data-action="increase"]');
    if (increaseBtn) {
      updateQuantity(increaseBtn.dataset.bookId, 1, increaseBtn.dataset.condition);
      return;
    }
    const decreaseBtn = event.target.closest('[data-action="decrease"]');
    if (decreaseBtn) {
      updateQuantity(decreaseBtn.dataset.bookId, -1, decreaseBtn.dataset.condition);
      return;
    }
    const removeBtn = event.target.closest('[data-action="remove"]');
    if (removeBtn) {
      removeFromCart(removeBtn.dataset.bookId, removeBtn.dataset.condition);
      return;
    }
    const closeModalBtn = event.target.closest('[data-action="close-modal"]');
    if (closeModalBtn) {
      els.bookModal.close();
      return;
    }

    // 2b. Intercept back-to-storefront clicks
    const backBtn = event.target.closest('[data-action="back-to-storefront"]');
    if (backBtn) {
      document.getElementById("storefrontView").hidden = false;
      document.getElementById("bookDetailsView").hidden = true;
      return;
    }

    // 2c. Intercept add-copy clicks from copies table
    const addCopyBtn = event.target.closest('[data-action="add-copy"]');
    if (addCopyBtn) {
      const bookId = addCopyBtn.dataset.bookId;
      const cond = addCopyBtn.dataset.condition;
      const price = parseFloat(addCopyBtn.dataset.price);
      addToCart(bookId, cond, price);
      return;
    }

    // 3. Intercept details card clicks (opens book details view page) - clicking anywhere on the card
    const detailsBtn = event.target.closest('[data-action="details"]');
    if (detailsBtn) {
      if (event.target.closest('.card-btn, .small-action, .remove-button, .stepper button')) return;
      showBookDetails(detailsBtn.dataset.bookId);
      return;
    }

    // 4. Intercept tabs clicking inside the modal or details view page
    const tabButton = event.target.closest("[data-tab-target]");
    if (tabButton) {
      const targetId = tabButton.dataset.tabTarget;
      const detailsContainer = tabButton.closest(".modal-details") || tabButton.closest(".details-right");
      if (detailsContainer) {
        detailsContainer.querySelectorAll(".modal-tab-button").forEach((btn) => {
          btn.classList.toggle("is-active", btn === tabButton);
        });
        detailsContainer.querySelectorAll(".modal-tab-content").forEach((content) => {
          content.classList.toggle("is-active", content.id === targetId);
        });
      }
      return;
    }

    // 5. Intercept carousel controls prev/next
    const carouselBtn = event.target.closest("[data-carousel-prev], [data-carousel-next]");
    if (carouselBtn) {
      const shelfId = carouselBtn.dataset.carouselPrev || carouselBtn.dataset.carouselNext;
      const shelf = document.getElementById(shelfId);
      if (shelf) {
        const scrollAmount = shelf.clientWidth * 0.75;
        const direction = carouselBtn.hasAttribute("data-carousel-prev") ? -1 : 1;
        shelf.scrollBy({ left: scrollAmount * direction, behavior: "smooth" });
      }
      return;
    }

    // 6. Close suggestions dropdown when clicking outside
    if (els.searchSuggestions && !els.headerSearchForm.contains(event.target) && !els.searchSuggestions.contains(event.target)) {
      els.searchSuggestions.hidden = true;
    }
  });

  els.cartToggle.addEventListener("click", openCart);
  els.closeCart.addEventListener("click", closeCart);
  els.cartDrawer.addEventListener("click", (event) => {
    if (event.target === els.cartDrawer) closeCart();
  });

  els.bookModal.addEventListener("click", (event) => {
    if (event.target === els.bookModal) els.bookModal.close();
  });

  document.addEventListener("keydown", (event) => {
    if (event.key === "Escape" && els.cartDrawer.classList.contains("is-open")) {
      closeCart();
    }
  });

  els.checkoutButton.addEventListener("click", () => {
    showToast("Demo checkout ready. Your used-book stack is saved in this browser.");
  });

  // Autocomplete suggestions item selection click handler
  els.searchSuggestions.addEventListener("click", (event) => {
    const removeHistoryBtn = event.target.closest(".history-remove");
    if (removeHistoryBtn) {
      event.stopPropagation();
      const historyItem = removeHistoryBtn.closest(".history-item");
      if (historyItem) historyItem.remove();
      return;
    }

    const categoryBtn = event.target.closest(".category-grid-btn");
    if (categoryBtn) {
      state.category = categoryBtn.dataset.category;
      els.searchSuggestions.hidden = true;
      renderBooks();
      document.querySelector("#catalog").scrollIntoView({ behavior: "smooth", block: "start" });
      return;
    }

    const recentItem = event.target.closest(".searchhub-recent-item");
    if (recentItem) {
      els.searchSuggestions.hidden = true;
      showBookDetails(recentItem.dataset.bookId);
      return;
    }

    const item = event.target.closest(".suggestion-item, .history-item");
    if (!item) return;
    const value = item.dataset.suggestionValue;
    els.headerSearchInput.value = value;
    els.sidebarSearchInput.value = value;
    state.query = value;
    els.searchSuggestions.hidden = true;
    renderBooks();
    document.querySelector("#catalog").scrollIntoView({ behavior: "smooth", block: "start" });
  });

  // Bind scroll listeners to product row carousels to auto-enable/disable navigation arrows
  const carouselRows = document.querySelectorAll(".product-row");
  carouselRows.forEach((row) => {
    row.addEventListener("scroll", () => {
      updateCarouselButtons(row);
    });
    // Set initial boundaries check
    setTimeout(() => updateCarouselButtons(row), 150);
  });
}

function initHeroCarousel() {
  const track = document.getElementById("heroCarouselTrack");
  if (!track) return;
  const slides = Array.from(track.querySelectorAll(".hero-slide"));
  const dots = Array.from(document.querySelectorAll("#heroCarouselDots .dot"));
  const prevBtn = document.getElementById("heroPrevBtn");
  const nextBtn = document.getElementById("heroNextBtn");
  
  let activeIndex = 0;
  let timer = null;

  const showSlide = (index) => {
    slides[activeIndex].classList.remove("active");
    dots[activeIndex].classList.remove("active");
    
    activeIndex = (index + slides.length) % slides.length;
    
    slides[activeIndex].classList.add("active");
    dots[activeIndex].classList.add("active");
  };

  const startTimer = () => {
    clearInterval(timer);
    timer = setInterval(() => {
      showSlide(activeIndex + 1);
    }, 6000);
  };

  if (prevBtn) {
    prevBtn.addEventListener("click", () => {
      showSlide(activeIndex - 1);
      startTimer();
    });
  }

  if (nextBtn) {
    nextBtn.addEventListener("click", () => {
      showSlide(activeIndex + 1);
      startTimer();
    });
  }

  dots.forEach((dot, idx) => {
    dot.addEventListener("click", () => {
      showSlide(idx);
      startTimer();
    });
  });

  startTimer();
}

populateGenreFilter();
renderStorefront();
renderMiniShelf();
renderBooks();
renderCart();
wireEvents();
initHeroCarousel();
