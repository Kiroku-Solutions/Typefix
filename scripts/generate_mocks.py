import urllib.request
import json
import os

def fetch_and_save(url, output_path, is_freq_list=False):
    print(f"Fetching {url}...")
    req = urllib.request.Request(url, headers={'User-Agent': 'Mozilla/5.0', 'Accept-Charset': 'utf-8'})
    try:
        with urllib.request.urlopen(req) as response:
            bytes_data = response.read()
            lines = bytes_data.decode('utf-8', errors='replace').splitlines()
        
        words_list = []
        for line in lines:
            line = line.strip()
            if not line: continue
            
            if is_freq_list:
                parts = line.split()
                if parts:
                    words_list.append(parts[0])
            else:
                words_list.append(line)
                
            if len(words_list) >= 15000:
                break
                
        # Generate JSON with the expected nested structure
        dict_json = {
            "words": [
                {"word": w, "frequency": 1} 
                for w in words_list if len(w) > 1
            ]
        }
        
        with open(output_path, 'w', encoding='utf-8') as f:
            json.dump(dict_json, f, ensure_ascii=False, indent=2)
            
        print(f"Saved {len(dict_json['words'])} words to {output_path}")
    except Exception as e:
        print(f"Failed to process {url}: {e}")

os.makedirs('data/dictionaries', exist_ok=True)

# English
fetch_and_save(
    'https://raw.githubusercontent.com/first20hours/google-10000-english/master/google-10000-english-no-swears.txt',
    'data/dictionaries/en.json'
)

# Spanish
fetch_and_save(
    'https://raw.githubusercontent.com/hermitdave/FrequencyWords/master/content/2018/es/es_50k.txt',
    'data/dictionaries/es.json',
    is_freq_list=True
)

# Portuguese
fetch_and_save(
    'https://raw.githubusercontent.com/hermitdave/FrequencyWords/master/content/2018/pt/pt_50k.txt',
    'data/dictionaries/pt.json',
    is_freq_list=True
)
