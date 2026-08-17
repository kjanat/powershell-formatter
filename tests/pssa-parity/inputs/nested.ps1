$config = @{
Settings = @{
Nested = @(
1,
2,
@{ Deep = $true }
)
}
Run = {
if ($x) {
@{
K = (Get-Date)
}
}
}
}
$obj.foreach({
$_.Name
})
(Get-ChildItem |
Where-Object Name).Count
[PSCustomObject]@{
Prop = 1
Other = 2
}
$s = $(
Get-Date
Get-Random
)
