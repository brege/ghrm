from benchmarks import ghrm


MODES = ("git_excluded", "git_included")


def setup_cache():
    return ghrm.ref_scan_config()


class RefScan:
    params = [ghrm.REF_SCAN_REPOS, MODES]
    param_names = ["repo", "mode"]
    number = 1
    repeat = 7
    warmup_time = 0
    timeout = 90

    def setup(self, config, repo, mode):
        self.root = ghrm.ref_repo(repo)
        self.config = config
        self.extra_args = ["--config", self.config]
        if mode == "git_included":
            self.extra_args.append("--dangerously-traverse-excludes")

    def time_nav_ready(self, config, repo, mode):
        port = ghrm.free_port()
        proc = ghrm.start_server(self.root, port, extra_args=self.extra_args)
        try:
            ghrm.wait_for_nav(f"http://127.0.0.1:{port}")
        finally:
            ghrm.stop_server(proc)
