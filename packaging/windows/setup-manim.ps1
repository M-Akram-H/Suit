# Installs Manim for Suit, for the person who runs this: into a Python environment of its
# own in %LOCALAPPDATA%\Suit, which Suit looks into by itself. Nothing is installed for the
# whole computer by this script itself; where Python or LaTeX are missing it offers to get
# them with winget, and asks first.
#
#   setup-manim.ps1            install Manim, or bring it up to what this Suit expects
#   setup-manim.ps1 -Remove    take it away again

param([switch]$Remove)

$ErrorActionPreference = "Stop"
$own = Join-Path $env:LOCALAPPDATA "Suit"
$venv = Join-Path $own "venv"
$here = Split-Path -Parent $MyInvocation.MyCommand.Path

function Finish([int]$code) {
    Write-Host ""
    Read-Host "Press Enter to close this window" | Out-Null
    exit $code
}

function Agreed([string]$question) {
    $answer = Read-Host "$question [Y/n]"
    return ($answer -eq "" -or $answer -match "^[Yy]")
}

# A Python that was installed the usual way: asked of the Python launcher, looked for where
# the installer puts it, and only then whatever is called "python" (which, untouched, is
# only Windows' pointer to its Store).
function Find-Python {
    foreach ($version in "3.12", "3.13", "3.11") {
        if (Get-Command py -ErrorAction SilentlyContinue) {
            $found = & py "-$version" -c "import sys; print(sys.executable)" 2>$null
            if ($LASTEXITCODE -eq 0 -and $found) { return "$found".Trim() }
        }
        $usual = Join-Path $env:LOCALAPPDATA ("Programs\Python\Python" + $version.Replace(".", "") + "\python.exe")
        if (Test-Path $usual) { return $usual }
    }
    $python = Get-Command python -ErrorAction SilentlyContinue
    if ($python -and $python.Source -notlike "*WindowsApps*") { return $python.Source }
    return $null
}

if ($Remove) {
    if (Test-Path $venv) { Remove-Item -Recurse -Force $venv }
    Write-Host "Manim for Suit is removed from $venv."
    Finish 0
}

Write-Host "Setting up Manim for Suit"
Write-Host "-------------------------"

$python = Find-Python
if (-not $python) {
    Write-Host "There is no Python on this computer. Suit needs Python 3.12."
    if ((Get-Command winget -ErrorAction SilentlyContinue) -and (Agreed "Install Python 3.12 now, with winget?")) {
        winget install -e --id Python.Python.3.12 --scope user
        $python = Find-Python
    }
    if (-not $python) {
        Write-Host "Install Python 3.12 from https://www.python.org/downloads/ and run this again."
        Finish 1
    }
}
$version = & $python -c "import sys; print('%d.%d' % sys.version_info[:2])"
Write-Host "Using Python $version ($python)"
Write-Host "Manim goes into $venv"
Write-Host "This downloads about 300 MB and takes a few minutes."
Write-Host ""

New-Item -ItemType Directory -Force $own | Out-Null
& $python -m venv --clear $venv
if ($LASTEXITCODE -ne 0) {
    Write-Host "That Python could not make an environment of its own."
    Finish 1
}
$inside = Join-Path $venv "Scripts\python.exe"
& $inside -m pip install --quiet --upgrade pip

# First with exactly the packages all of Suit's tests pass with; if one of those cannot be
# had for Windows, with the two that matter and whatever goes with them now.
$requirements = Join-Path $here "requirements.txt"
$constraints = Join-Path $here "constraints.txt"
$installed = $false
if ($version -eq "3.12" -and (Test-Path $constraints)) {
    & $inside -m pip install -r $requirements -c $constraints
    $installed = ($LASTEXITCODE -eq 0)
    if (-not $installed) { Write-Host "Trying once more, with the newest matching packages..." }
}
if (-not $installed) {
    & $inside -m pip install -r $requirements
    if ($LASTEXITCODE -ne 0) {
        Write-Host ""
        Write-Host "Manim could not be installed. What pip said is above."
        Finish 1
    }
}

Write-Host ""
& $inside -c "import manim, manim_slides; print('Manim', manim.__version__, 'and manim-slides', manim_slides.__version__, 'are installed.')"
if ($LASTEXITCODE -ne 0) {
    Write-Host "Manim was installed but does not start."
    Finish 1
}

# LaTeX, which Manim typesets formulas with. Texts, shapes and graphs work without it.
if (-not (Get-Command latex -ErrorAction SilentlyContinue)) {
    Write-Host ""
    Write-Host "LaTeX is not installed, so formulas will not be drawn yet."
    if ((Get-Command winget -ErrorAction SilentlyContinue) -and (Agreed "Install MiKTeX (a LaTeX for Windows) now, with winget?")) {
        winget install -e --id MiKTeX.MiKTeX
        $env:Path += ";" + (Join-Path $env:LOCALAPPDATA "Programs\MiKTeX\miktex\bin\x64")
    } else {
        Write-Host "You can install it later from https://miktex.org/download"
    }
}
# MiKTeX fetches the LaTeX packages a formula needs when it first needs them, and by
# itself asks before each one, in a window Suit cannot show. It is told not to ask.
if (Get-Command initexmf -ErrorAction SilentlyContinue) {
    initexmf --set-config-value "[MPM]AutoInstall=1" 2>$null
    Write-Host "MiKTeX will fetch the LaTeX packages formulas need by itself. The first formula takes a while."
}

Write-Host ""
Write-Host "Suit is ready. Start it from the Start menu."
Finish 0
