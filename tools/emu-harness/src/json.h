// SPDX-License-Identifier: GPL-3.0-only
// Minimal JSON reader for harness scripts (objects, arrays, strings, numbers, bools, null).
#pragma once

#include <map>
#include <memory>
#include <stdexcept>
#include <string>
#include <vector>

namespace json {

struct Value
{
    enum Kind { Null, Bool, Number, String, Array, Object } kind = Null;
    bool b = false;
    double n = 0;
    std::string s;
    std::vector<Value> a;
    std::map<std::string, Value> o;

    bool has(const std::string& k) const { return kind == Object && o.count(k); }
    const Value& operator[](const std::string& k) const;
    const Value& operator[](size_t i) const { return a.at(i); }
    long long i() const { return (long long)n; }
};

Value parse(const std::string& text);

}  // namespace json
