# leading file comment
function Get-Widget {
    # inner comment
    param(
        [int] $Count # trailing comment
    )
    <# block
       comment #>
    $Count # after code
}
foreach ($x in $y) { # loop comment
    $x
}
if ($cond) { 'a' } # one-liner comment
$h = @{
    Key = 'v' # entry comment
    K2  = 'w'
}
