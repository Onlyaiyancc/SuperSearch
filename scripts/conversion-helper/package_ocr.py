"""兼容旧打包命令；公开 OCR 组件一律从固定的官方源码构建。"""

from pathlib import Path
import runpy

if __name__ == '__main__':
    runpy.run_path(str(Path(__file__).with_name('public-ocr-build.py')), run_name='__main__')
