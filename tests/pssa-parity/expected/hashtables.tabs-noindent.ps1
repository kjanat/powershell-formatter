$one = @{ a = 1; b = 2 }
$two = @{
	Name       = 'x'
	LongerName =    'y'
	S          = 'z'
}
$nested = @{
	Outer = @{
		In     = 1
		Inner2 = 22
	}
	B     = 2
}
$splat = @{
	ComputerName ='server'
	Credential   =  $cred
	Port         = 8080
}
enum Color {
	Red        = 1
	Green      = 22
	BlueViolet = 333
	Plain
}
