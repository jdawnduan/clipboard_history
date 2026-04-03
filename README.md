# Documentation

## Interactive Course: How Clipboard History Works

A beautiful, interactive course that teaches how the clipboard-history codebase works — no coding knowledge required.

### View the Course

Open `course/index.html` in your browser:
```bash
# On macOS
open docs/course/index.html

# Or just double-click the file in Finder
```

### What You'll Learn

The course covers:
- **Module 1**: What happens when you press the hotkey
- **Module 2**: Meet the components (Clipboard, History, Monitor, Platform)
- **Module 3**: The data pipeline and concurrency
- **Module 4**: The clever engineering tricks
- **Module 5**: Debugging when things go wrong

### For Developers

If you want to regenerate the course HTML after making changes:

```bash
cd docs/course
bash build.sh
```

This assembles `index.html` from the module files in `modules/`.

### Course Structure

```
course/
├── index.html      # Assembled course (open this in browser)
├── build.sh        # Build script
├── styles.css      # Design system
├── main.js         # Interactive elements engine
├── _base.html      # HTML shell
├── modules/        # Individual module HTML files
│   ├── 01-intro.html
│   ├── 02-actors.html
│   ├── 03-pipeline.html
│   ├── 04-tricks.html
│   └── 05-debug.html
```

### Tech Stack

The course is built with:
- Pure HTML/CSS/JavaScript (no dependencies)
- Google Fonts (Bricolage Grotesque, DM Sans, JetBrains Mono)
- Custom interactive elements (chat animations, data flow diagrams, quizzes)

Built with [codebase-to-course](https://github.com/zarazhangrui/codebase-to-course) written by [zarazhangrui](https://github.com/zarazhangrui).
