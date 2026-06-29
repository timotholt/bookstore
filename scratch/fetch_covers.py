import os
import shutil
import sqlite3
import urllib.request
import struct

# Paths
DB_PATH = "data/bookstore.db"
COVERS_DIR = "assets/covers"
PLACEHOLDER_SRC = "/Users/timotholt/.gemini/antigravity/brain/7883a8cc-6cc4-4849-ac90-04909a0d5ed2/placeholder_cover_1782675002997.jpg"
PLACEHOLDER_DEST = os.path.join(COVERS_DIR, "placeholder.jpg")

def get_jpeg_dimensions(data):
    idx = 0
    while idx < len(data) - 9:
        if data[idx] == 0xff and data[idx+1] in (0xc0, 0xc1, 0xc2, 0xc3, 0xc5, 0xc6, 0xc7, 0xc9, 0xca, 0xcb, 0xcd, 0xce, 0xcf):
            try:
                # Height and width are at idx+5 and idx+7
                h, w = struct.unpack(">HH", data[idx+5:idx+9])
                return w, h
            except Exception:
                pass
        idx += 1
    return None

def main():
    # 1. Create covers directory
    os.makedirs(COVERS_DIR, exist_ok=True)

    # 2. Copy placeholder cover
    if os.path.exists(PLACEHOLDER_SRC):
        shutil.copy(PLACEHOLDER_SRC, PLACEHOLDER_DEST)
        print(f"Copied placeholder to {PLACEHOLDER_DEST}")
    else:
        print(f"Warning: Placeholder source {PLACEHOLDER_SRC} not found!")

    # 3. Read placeholder dimensions to get its aspect ratio
    placeholder_aspect = 0.667
    if os.path.exists(PLACEHOLDER_DEST):
        with open(PLACEHOLDER_DEST, "rb") as f:
            p_data = f.read()
            dims = get_jpeg_dimensions(p_data)
            if dims:
                placeholder_aspect = round(dims[0] / dims[1], 3)
                print(f"Placeholder aspect ratio: {placeholder_aspect} (dims: {dims[0]}x{dims[1]})")

    # 4. Connect to DB
    conn = sqlite3.connect(DB_PATH)
    cursor = conn.cursor()
    cursor.execute("SELECT id, title, isbn FROM books")
    books = cursor.fetchall()

    print(f"Found {len(books)} books in database.")

    headers = {
        'User-Agent': 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/58.0.3029.110 Safari/537.3'
    }

    for book_id, title, isbn in books:
        isbn = isbn.strip()
        print(f"\nProcessing [{book_id}] '{title}' (ISBN: {isbn})...")
        dest_path = os.path.join(COVERS_DIR, f"{book_id}.jpg")

        url = f"https://covers.openlibrary.org/b/isbn/{isbn}-L.jpg?default=false"
        req = urllib.request.Request(url, headers=headers)
        
        success = False
        try:
            with urllib.request.urlopen(req, timeout=10) as response:
                content = response.read()
                # Check for redirects or actual content
                if response.status == 200 and len(content) > 100:
                    dims = get_jpeg_dimensions(content)
                    if dims and dims[0] > 5 and dims[1] > 5:
                        with open(dest_path, "wb") as f:
                            f.write(content)
                        aspect = round(dims[0] / dims[1], 3)
                        print(f"  -> Downloaded! Dims: {dims[0]}x{dims[1]}, Aspect Ratio: {aspect}")
                        cursor.execute("UPDATE books SET aspect_ratio = ? WHERE id = ?", (aspect, book_id))
                        success = True
                    else:
                        print(f"  -> Warning: Invalid dimensions found in response.")
                else:
                    print(f"  -> Warning: Response status {response.status} or small content length {len(content)}")
        except Exception as e:
            print(f"  -> Failed to fetch cover: {e}")

        if not success:
            print(f"  -> Using placeholder cover.")
            shutil.copy(PLACEHOLDER_DEST, dest_path)
            cursor.execute("UPDATE books SET aspect_ratio = ? WHERE id = ?", (placeholder_aspect, book_id))

    conn.commit()
    conn.close()
    print("\nCover synchronization and database updates completed successfully!")

if __name__ == "__main__":
    main()
