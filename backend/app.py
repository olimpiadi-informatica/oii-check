from flask import Flask, render_template, request
# from flask_cors import CORS, cross_origin
import base64
import time
import json
import os

app = Flask(__name__, static_folder='assets', static_url_path='/assets')
DATA_FOLDER = "data"
if not os.path.exists(DATA_FOLDER):
    os.makedirs(DATA_FOLDER)
TOKENS_FILE = "partecipation.json"
milli_time = lambda: int(round(time.time() * 1000))
# CORS(app)
# app.config['CORS_HEADERS'] = 'Content-Type'

# Load tokens from file
if not os.path.exists(TOKENS_FILE):
    app.logger.error(f"Tokens file not found. Please create '{TOKENS_FILE}'. Exiting.")
    exit(1)
tokens = json.load(open(TOKENS_FILE))
tokens_dict = {}
for token in tokens:
    tokens_dict[token['token']] = token['type']
app.logger.info(f"Tokens from '{TOKENS_FILE}' loaded successfully.")

json_ok = json.dumps({"status": "ok"})
json_error = json.dumps({"status": "error"})

# Main page with token request + dynamic links + injection of js
@app.route('/')
def index():
    return render_template('index.html')

def _verify_token(token):
    if token not in tokens_dict:
        return False
    return True

# Token verification page
@app.route('/verify_token', methods=['POST'])
def verify_token():
    data = request.get_json()
    if 'token' not in data:
        return json_error
    token = data['token']
    if not _verify_token(token):
        return json_error
    token_type = tokens_dict[token]
    app.logger.info(f"Token '{token}' verified successfully. Type: {token_type}")
    return json.dumps({"status": "ok", "type": token_type})

def check_token_path(token):
    # Do not log empty or None tokens
    if token is None or token == "":
        return False
    # if not _verify_token(token):
    #     return False
    if ".." in token or "/" in token:
        return False
    directory = os.path.join(DATA_FOLDER, token)
    if not os.path.exists(directory):
        os.makedirs(directory)
    return True

@app.route('/internet', methods=['POST'])
def internet():
    data = request.get_json()
    if any(key not in data for key in ['fp', 'ic', 'mid', 'ts']):
        return json_error
    token = data['mid']
    if not check_token_path(token):
        return json_error
    fingerprint = data['fp']
    internet_check = data['ic']
    timestamp = data['ts']

    # Add server timestamp
    data['server_ts'] = milli_time()

    filename = os.path.join(DATA_FOLDER, token, "internet.json")
    with open(filename, "a") as f:
        f.write(json.dumps(data) + "\n")
    return json_ok

@app.route('/screen', methods=['POST'])
def screen():
    data = request.get_json()
    if any(key not in data for key in ['img', 'mid']):
        return json_error
    token = data['mid']
    if not check_token_path(token):
        return json_error

    img = data['img']
    img = base64.b64decode(img)

    filename = os.path.join(DATA_FOLDER, token, str(int(time.time())) + ".webp")
    with open(filename, "wb") as f:
        f.write(img)
    return json_ok

if __name__ == '__main__':
    app.run(debug=True)

