#!/bin/python
import os
import sys
from wordlist_combined import WordlistCombined, DictionaryHeader
from wordlist import Wordlist, hun_loc
from spylls.hunspell import Dictionary

locale = sys.argv[1]
print("building dictionary for " + locale)
d = Dictionary.from_files(os.path.dirname(os.path.abspath(__file__)) + "/dicts/" + hun_loc(locale, False))
w = Wordlist(dictionary=d)
w.add_words_from_dictionary()
c = w.create_wordlist_combined()
c.header = DictionaryHeader(locale, "main", sys.argv[4], int(sys.argv[2]), int(sys.argv[3]))
#c.write_to_file(os.path.dirname(os.path.abspath(__file__)) + "/LIME-dictionaries/" + locale + "_wordlist.compiled")
c.compile(os.path.dirname(os.path.abspath(__file__)) + "/../../LIME/main_" + locale.lower() + ".dict")