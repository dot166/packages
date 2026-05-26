#!/bin/bash
git clone https://github.com/utuhiro78/merge-ut-dictionaries.git
cd merge-ut-dictionaries
git am ../../../0001-enable-all-dicts.patch
cd src/merge
chmod +x make.sh
./make.sh
cat mozcdic-ut.txt >> ../../../data/dictionary_oss/dictionary00.txt
cp ../../../data/dictionary_oss/dictionary00.txt ../../../../../dicts/mozcdic-ut.txt