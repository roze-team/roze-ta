"""Compile pinned GPL TTR C routines as a test-only R DLL, never linked to Rust."""
import os,re,subprocess,sys
sys.stdout.reconfigure(encoding='utf8')
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
from parity_sources import verify_source
verify_source('ttr')
BUILD=ROOT/'target/cross-library/ttr-msvc'
BUILD.mkdir(parents=True,exist_ok=True)
VC=Path('C:/Program Files/Microsoft Visual Studio/2022/Community/VC/Tools/MSVC/14.44.35207/bin/Hostx64/x64')
env={k.upper():v for k,v in os.environ.items()};env['PLATFORM']='x64'
def run(args):return subprocess.run(args,env=env,cwd=BUILD,check=True,capture_output=True,text=True,errors='replace').stdout
exports=run([str(VC/'dumpbin.exe'),'/exports',str(ROOT/'target/R-runtime/app/bin/x64/R.dll')])
names=re.findall(r'^\s+\d+\s+[0-9A-F]+\s+[0-9A-F]+\s+(\S+)',exports,re.M)
assert len(names)>100
(BUILD/'R.def').write_text('LIBRARY R.dll\nEXPORTS\n'+'\n'.join(names)+'\n')
run([str(VC/'lib.exe'),'/def:R.def','/out:R.lib','/machine:x64'])
source=ROOT/'target/cross-library/ttr/src';rinclude=ROOT/'target/R-runtime/app/include'
cmake='cmake_minimum_required(VERSION 3.20)\nproject(TTR_reference C)\nadd_library(TTR SHARED\n'+''.join('"'+p.as_posix()+'"\n' for p in source.glob('*.c'))+')\n'
cmake+=f'target_include_directories(TTR PRIVATE "{rinclude.as_posix()}")\ntarget_link_libraries(TTR PRIVATE "{(BUILD/"R.lib").as_posix()}")\ntarget_compile_options(TTR PRIVATE /utf-8 /std:c11)\nset_target_properties(TTR PROPERTIES WINDOWS_EXPORT_ALL_SYMBOLS ON)\n'
(BUILD/'msvc-r-headers.h').write_text('#include <Rconfig.h>\n#undef HAVE_ENUM_BASE_TYPE\n#define R_LEGACY_RCOMPLEX 1\n')
cmake+=f'target_compile_options(TTR PRIVATE "/FI{(BUILD/"msvc-r-headers.h").as_posix()}")\n'
(BUILD/'CMakeLists.txt').write_text(cmake)
try:
    print(run(['cmake','-S',str(BUILD),'-B',str(BUILD/'build')]))
    output=run(['cmake','--build',str(BUILD/'build'),'--config','Release','--parallel','4'])
    (BUILD/'build.log').write_text(output,encoding='utf8')
    print('Build completed; details in',BUILD/'build.log')
except subprocess.CalledProcessError as e:
    (BUILD/'build-failure.log').write_text((e.stdout or '')+(e.stderr or ''),encoding='utf8')
    print('\n'.join(line for line in (e.stdout or '').splitlines() if 'error ' in line)[:6000]);raise
print(BUILD/'build/Release/TTR.dll')
