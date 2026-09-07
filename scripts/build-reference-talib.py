"""Build the pinned C reference oracle, outside the Rust production workspace."""
import os
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
from parity_sources import verify_source
verify_source('ta-lib')
# The host may export both Path and PATH. MSBuild rejects case duplicates.
env = {key.upper():value for key,value in os.environ.items()}
env["PLATFORM"] = "x64"
commands = [
    ["cmake","-S","target/cross-library/ta-lib","-B","target/cross-library/ta-lib-msvc","-DBUILD_SHARED_LIBS=ON","-DBUILD_STATIC_LIBS=OFF","-DBUILD_DEV_TOOLS=OFF","-DBUILD_BENCHMARKS=OFF","-DCMAKE_C_FLAGS=/utf-8","-DCMAKE_CXX_FLAGS=/utf-8"],
    ["cmake","--build","target/cross-library/ta-lib-msvc","--config","Release","--parallel","4"],
]
with (ROOT/"target/cross-library/ta-lib-build.log").open("w",encoding="utf-8") as log:
    for command in commands:
        subprocess.run(["rtk","proxy",*command],cwd=ROOT,env=env,stdout=log,stderr=subprocess.STDOUT,check=True)
print("Built pinned TA-Lib C oracle; output: target/cross-library/ta-lib-build.log")
