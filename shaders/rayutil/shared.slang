#pragma once

#define ENABLE_ERROR_TRACE 1

public struct Ray{
    public float3 dir;
    public float3 origin;
};

namespace Error{
    
    enum Raytrace{
        HashMapFailure = 0,
        Err2, // TODO
        Err1, // TODO
        Count,
    };


    float4 get_error_color(Error::Raytrace err){
        float4 err_colors[Raytrace.Count] = {
            float4(1.0, 0.0, 0.0, 1.0), // Red
            float4(0.0, 1.0, 0.0, 1.0), // Green
            float4(0.0, 0.0, 1.0, 1.0)  // Blue
        };

        return err_colors[err];
    }

   
    #if(ENABLE_ERROR_TRACE)
        Error::Raytrace err_ret<T>(Error::Raytrace err, T val){
            return err;
        }
    #else
        T err_ret<T>(Error::Raytrace err, T val){
            return val;
        }
    #endif

}

#if(ENABLE_ERROR_TRACE)
    #define ErrThrow throws Error::Raytrace
    #define ErrReturn throw
    #define ErrVoid throw
    #define Ret
    #define ErrTry try
#else
    #define ErrThrow
    #define ErrVoid
    #define ErrReturn return
    #define Ret return
    #define Err try
#endif

