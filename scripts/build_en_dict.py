import urllib.request
import json
import ssl
import sys

ssl._create_default_https_context = ssl._create_unverified_context

url = "https://raw.githubusercontent.com/hermitdave/FrequencyWords/master/content/2018/en/en_50k.txt"
print(f"Downloading {url}...")
try:
    response = urllib.request.urlopen(url)
    data = response.read().decode('utf-8')
except Exception as e:
    print(f"Error downloading: {e}")
    sys.exit(1)

words_array = []
added = set()

# Always add some single-letter words that are common
words_array.append({"word": "a", "frequency": 100000000})
words_array.append({"word": "I", "frequency": 100000000})
added.add("a")
added.add("i")

for line in data.split('\n'):
    line = line.strip()
    if not line: continue
    parts = line.split(' ')
    if len(parts) == 2:
        word = parts[0]
        freq = int(parts[1])
        if word.isalpha() and len(word) > 1 and word.lower() not in added:
            words_array.append({"word": word, "frequency": freq})
            added.add(word.lower())

output_path = "data/dictionaries/en.json"
print(f"Writing {len(words_array)} words to {output_path}...")
with open(output_path, "w", encoding="utf-8") as f:
    json.dump({"words": words_array}, f, indent=2)

print("Done!")
