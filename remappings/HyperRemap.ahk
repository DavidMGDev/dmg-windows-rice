#Requires AutoHotkey v2.0
#SingleInstance

; --- Permanently disable Caps Lock ---
SetCapsLockState("AlwaysOff")

!,::
{
    Send("{Text}<")
}

!.::
{
    Send("{Text}>")
}

; --- Hyper Key Mappings ---
CapsLock & q::Send("{Text}@")
CapsLock & e::Send("{Text}=")
CapsLock & w::Send("{Text}#")
CapsLock & r::Send("{Text}$")
CapsLock & p::Send("{Text}%")
CapsLock & y::Send("{Text}^")
CapsLock & o::Send("{Text}/")
CapsLock & u::Send("{Text}\")
CapsLock & t::Send("{Text}~")
CapsLock & i::Send("{Text}|")
CapsLock & s::Send("{Text}&")
CapsLock & a::Send("{Text}*")
CapsLock & h::Send("{Text}()")
CapsLock & j::Send("{Text}{}")
CapsLock & k::Send("{Text}[]")
CapsLock & d::Send("{Text}'")
CapsLock & f::Send("{Text}-")
CapsLock & g::Send("{Text}+")
CapsLock & l::Send("{Text}¯\_(ツ)_/¯ ")
CapsLock & b::Send("{Text}✓")
CapsLock & n::Send("{Text}ಥ_ಥ")
CapsLock & m::Send("{Text}( ͡° ͜ʖ ͡°)")
CapsLock & z::Send("{Text}♠")
CapsLock & x::Send("{Text}♣")
CapsLock & c::Send("{Text}♥")
CapsLock & v::Send("{Text}♦")