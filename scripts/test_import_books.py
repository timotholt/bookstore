import importlib.util
import tempfile
import unittest
from pathlib import Path

spec=importlib.util.spec_from_file_location('import_books',Path(__file__).with_name('import_books.py'))
module=importlib.util.module_from_spec(spec); spec.loader.exec_module(module)

class ImportGates(unittest.TestCase):
    def test_isbn_checksum_and_conversion(self):
        self.assertEqual(module.isbn13('0-306-40615-2'),'9780306406157')
        self.assertEqual(module.isbn13('9780306406157'),'9780306406157')
        self.assertIsNone(module.isbn13('9780306406158'))
        self.assertIsNone(module.isbn13('1234567890'))
        self.assertIsNone(module.isbn13('0000000000000'))

    def test_missing_description_rejected(self):
        self.assertIsNone(module.candidate({'isbn_13':['9780306406157'],'title':'A book'}))

    def test_missing_or_invalid_image_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            row={'cover_source':'https://covers.openlibrary.org/b/id/123-M.jpg?default=false'}
            self.assertIsNone(module.verify_cover(row,Path(directory)))
            Path(directory,'123.jpg').write_text('not an image')
            self.assertIsNone(module.verify_cover(row,Path(directory)))

if __name__=='__main__': unittest.main()
