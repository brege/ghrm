from pathlib import Path

from benchmarks import ghrm


CASE_PARAMS = ("name", "size", "lines")


def setup_cache():
    return ghrm.path_search_medium()


class PathSearch:
    params = CASE_PARAMS
    param_names = ["sort"]
    number = 1
    repeat = 10
    warmup_time = 0.1
    timeout = 30

    def setup(self, fixture, sort):
        self.root = Path(fixture)
        self.port = ghrm.free_port()
        self.proc = ghrm.start_server(self.root, self.port)
        self.base = f"http://127.0.0.1:{self.port}"
        ghrm.wait_for_nav(self.base)
        self.url = ghrm.path_search_url(self.base, sort)

    def teardown(self, fixture, sort):
        ghrm.stop_server(self.proc)

    def time_query(self, fixture, sort):
        response = ghrm.fetch_json(self.url)
        if response.get("pending"):
            raise RuntimeError("path search nav is pending")
        if not response.get("results"):
            raise RuntimeError("path search returned no rows")

    def track_rows(self, fixture, sort):
        response = ghrm.fetch_json(self.url)
        return len(response["results"])
