import ast,sys
from pathlib import Path
root=Path(__file__).resolve().parents[1]/'target/cross-library'
for name in sys.argv[1:]:
    for file in (root/'ta/ta').glob('*.py'):
        tree=ast.parse(file.read_text(encoding='utf8'))
        for node in tree.body:
            if isinstance(node,ast.ClassDef) and node.name==name:
                print(name)
                for fn in node.body:
                    if isinstance(fn,ast.FunctionDef) and fn.name=='_run':print(ast.unparse(fn))
