# --- Configuration ---
$NumRuns = 10
$PackageName = "kcd_bilingual_generator_rust"
# $TestFunctionName = "test_generate_multithread"
$TestFunctionName = "test_generate_async"
# Important: Ensure cargo is in your PATH
$CargoCommandPath = (Get-Command cargo).Source # More robust way to find cargo
if (-not $CargoCommandPath) {
    Write-Error "Could not find 'cargo' in your PATH. Please ensure the Rust toolchain is installed correctly."
    exit 1
}

# Construct the arguments array
$CargoArgsArray = @(
    "test",
    "--release",
    "--package", $PackageName,
    "--test", 
    $TestFunctionName, # The test binary name
    "--", # Separator for test harness arguments
    "tests::$TestFunctionName", # The specific test function path
    "--exact", # Ensure only this test runs
    "--show-output"                                             # Capture stdout/stderr from the test
)

# --- Helper Function to Quote Arguments for ProcessStartInfo.Arguments ---
function Format-Argument {
    param($Argument)
    # If argument contains space, tab, or double quote, enclose in double quotes.
    # Also, double up any existing double quotes within the argument.
    if ($Argument -match '[ "`]|\t') {
        '"' + ($Argument -replace '"', '""') + '"'
    }
    else {
        $Argument
    }
}

# --- Script Logic ---
Write-Host "Starting performance test for '$TestFunctionName'..."
Write-Host "Running $NumRuns times..."
Write-Host "Using Cargo Command: $CargoCommandPath"

$SuccessTimes = @() # Array to store durations of successful runs in seconds

for ($i = 1; $i -le $NumRuns; $i++) {
    Write-Host "`n--- Running Test [$i / $NumRuns] ---"
    $startTime = Get-Date

    # Escape arguments for the .Arguments string property
    $escapedArgs = $CargoArgsArray | ForEach-Object { Format-Argument $_ }
    $argumentsString = $escapedArgs -join ' '
    # Write-Host "Executing: $CargoCommandPath $argumentsString" # Uncomment for debugging command line

    # Configure ProcessStartInfo
    $processInfo = New-Object System.Diagnostics.ProcessStartInfo
    $processInfo.FileName = $CargoCommandPath
    $processInfo.Arguments = $argumentsString  # Use the joined, quoted string
    $processInfo.RedirectStandardError = $true
    $processInfo.RedirectStandardOutput = $true
    $processInfo.UseShellExecute = $false
    $processInfo.CreateNoWindow = $true
    # $processInfo.WorkingDirectory = "C:\path\to\your\project" # Set if needed, defaults to script's dir

    # Create and Start the Process
    $process = New-Object System.Diagnostics.Process
    $process.StartInfo = $processInfo

    # Use ScriptBlock for event handlers (often more reliable in PS)
    $outputBuilder = New-Object System.Text.StringBuilder
    $outputHandler = { if ($EventArgs.Data) { $outputBuilder.AppendLine($EventArgs.Data) | Out-Null } }
    $errorHandler = { if ($EventArgs.Data) { $outputBuilder.AppendLine($EventArgs.Data) | Out-Null } } # Append errors too

    # Register event handlers BEFORE starting async reads
    Register-ObjectEvent -InputObject $process -EventName OutputDataReceived -Action $outputHandler -SourceIdentifier "CargoOutput$i" | Out-Null
    Register-ObjectEvent -InputObject $process -EventName ErrorDataReceived -Action $errorHandler -SourceIdentifier "CargoError$i" | Out-Null

    try {
        # Start the process
        $process.Start() | Out-Null

        # Begin asynchronous reading
        $process.BeginOutputReadLine()
        $process.BeginErrorReadLine()

        # Wait for the process to complete
        $process.WaitForExit()
        $exitCode = $process.ExitCode
    }
    catch {
        Write-Error "Failed to start or monitor process for run $i : $_"
        # Optional: Mark run as failed and continue or stop script
        $exitCode = -1 # Indicate failure
    }
    finally {
        # IMPORTANT: Unregister event handlers to avoid memory leaks in loops
        Unregister-Event -SourceIdentifier "CargoOutput$i" -ErrorAction SilentlyContinue
        Unregister-Event -SourceIdentifier "CargoError$i" -ErrorAction SilentlyContinue
        # Ensure process object is disposed
        if ($null -ne $process) {
            $process.Dispose()
        }
    }


    $endTime = Get-Date
    $runDuration = $endTime - $startTime
    $output = $outputBuilder.ToString()

    # Write the full output for debugging if needed
    # Write-Host "Raw Output:"
    # Write-Host $output

    Write-Host "Test execution finished in $($runDuration.TotalSeconds) seconds (wall clock time)."

    # Check if the test passed based on exit code and output line
    if ($exitCode -eq 0 -and $output -match 'test result: ok\. 1 passed') {
        # Escaped dot in regex
        Write-Host "Test run $i PASSED."
        # Try to extract the time reported by cargo
        if ($output -match 'finished in ([\d.]+?)s') {
            $timeString = $matches[1]
            try {
                # Use InvariantCulture for reliable decimal parsing
                $timeSeconds = [double]::Parse($timeString, [System.Globalization.CultureInfo]::InvariantCulture)
                Write-Host "Cargo reported time: ${timeSeconds}s"
                $SuccessTimes += $timeSeconds
            }
            catch {
                Write-Warning "Run $i : Passed, but could not parse time '$timeString' from output. Skipping time for average calculation. Error: $($_.Exception.Message)"
            }
        }
        else {
            Write-Warning "Run $i : Passed, but could not find 'finished in ...s' pattern in output. Skipping time for average calculation."
            # Write-Host $output # Uncomment to see output if pattern fails
        }
    }
    else {
        Write-Error "Test run $i FAILED (Exit Code: $exitCode)."
        Write-Host "--- Full Output for Failed Run $i ---"
        Write-Host $output
        Write-Host "------------------------------------"
        # Optional: stop the script on first failure:
        # throw "Test run $i failed. Aborting."
    }

    # Small delay before next run, can sometimes help with resource cleanup if tests are heavy
    # Start-Sleep -Milliseconds 100

} # End of loop

# --- Calculate and Report Average ---
Write-Host "`n--- Performance Summary ---"
$successfulRuns = $SuccessTimes.Count

if ($successfulRuns -gt 0) {
    Write-Host "Successful runs: $successfulRuns / $NumRuns"
    Write-Host "Individual successful run times (seconds reported by Cargo):"
    $SuccessTimes | ForEach-Object { Write-Host ("  {0:N3}" -f $_) } # Format to 3 decimal places

    # Calculate average
    $totalTime = ($SuccessTimes | Measure-Object -Sum).Sum
    $averageTime = $totalTime / $successfulRuns

    Write-Host ("Total time reported by Cargo across successful runs: {0:N3}s" -f $totalTime)
    Write-Host ("Average time reported by Cargo per successful run: {0:N3}s" -f $averageTime)
}
else {
    Write-Warning "No test runs completed successfully. Cannot calculate average time."
}

Write-Host "--- Script Finished ---"

# Optional: Check if all runs were successful for a final status
if ($successfulRuns -ne $NumRuns) {
    Write-Warning "Note: Not all $NumRuns runs were successful."
    # Consider exiting with an error code if running in automation
    # exit 1
}
else {
    Write-Host "All $NumRuns runs completed successfully."
}