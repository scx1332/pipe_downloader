import requests


class FileLimiter(object):
    def __init__(self, file_obj, read_limit):
        self.read_limit = read_limit
        self.amount_seen = 0
        self.file_obj = file_obj

        # So that requests doesn't try to chunk the upload but will instead stream it:
        self.len = read_limit

    def read(self, amount=-1):
        if self.amount_seen >= self.read_limit:
            return b''
        remaining_amount = self.read_limit - self.amount_seen
        data = self.file_obj.read(min(amount, remaining_amount))
        self.amount_seen += len(data)
        return data


files = {'upload_file': open('Cargo.toml','rb')}


r = requests.post("http://127.0.0.1:5554/upload", files=files)
print("Response: {}".format(r))
print("Response text: {}".format(r.text))
