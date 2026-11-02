// SPDX-License-Identifier: GPL-3.0-only
#include "json.h"

#include <cctype>
#include <cstdlib>

namespace json {

const Value& Value::operator[](const std::string& k) const
{
    auto it = o.find(k);
    if (kind != Object || it == o.end())
        throw std::runtime_error("missing key: " + k);
    return it->second;
}

namespace {

struct Parser
{
    const std::string& t;
    size_t p = 0;

    [[noreturn]] void fail(const char* what)
    {
        throw std::runtime_error(std::string("json: ") + what + " at offset " + std::to_string(p));
    }

    void ws()
    {
        while (p < t.size())
        {
            if (isspace((unsigned char)t[p]))
                p++;
            else if (t.compare(p, 2, "//") == 0)  // allow line comments in scripts
                while (p < t.size() && t[p] != '\n')
                    p++;
            else
                break;
        }
    }

    bool eat(char c)
    {
        ws();
        if (p < t.size() && t[p] == c)
        {
            p++;
            return true;
        }
        return false;
    }

    std::string str()
    {
        if (!eat('"'))
            fail("expected string");
        std::string out;
        while (p < t.size() && t[p] != '"')
        {
            char c = t[p++];
            if (c == '\\' && p < t.size())
            {
                char e = t[p++];
                switch (e)
                {
                    case 'n': out += '\n'; break;
                    case 't': out += '\t'; break;
                    case 'u':
                    {
                        unsigned v = std::strtoul(t.substr(p, 4).c_str(), nullptr, 16);
                        p += 4;
                        if (v < 0x80)
                            out += (char)v;
                        else if (v < 0x800)
                        {
                            out += (char)(0xC0 | (v >> 6));
                            out += (char)(0x80 | (v & 0x3F));
                        }
                        else
                        {
                            out += (char)(0xE0 | (v >> 12));
                            out += (char)(0x80 | ((v >> 6) & 0x3F));
                            out += (char)(0x80 | (v & 0x3F));
                        }
                        break;
                    }
                    default: out += e;
                }
            }
            else
                out += c;
        }
        if (p >= t.size())
            fail("unterminated string");
        p++;
        return out;
    }

    Value value()
    {
        ws();
        if (p >= t.size())
            fail("unexpected end");
        Value v;
        char c = t[p];
        if (c == '{')
        {
            p++;
            v.kind = Value::Object;
            if (eat('}'))
                return v;
            do
            {
                ws();
                std::string k = str();
                if (!eat(':'))
                    fail("expected ':'");
                v.o[k] = value();
            } while (eat(','));
            if (!eat('}'))
                fail("expected '}'");
        }
        else if (c == '[')
        {
            p++;
            v.kind = Value::Array;
            if (eat(']'))
                return v;
            do
                v.a.push_back(value());
            while (eat(','));
            if (!eat(']'))
                fail("expected ']'");
        }
        else if (c == '"')
        {
            v.kind = Value::String;
            v.s = str();
        }
        else if (t.compare(p, 4, "true") == 0)
        {
            v.kind = Value::Bool;
            v.b = true;
            p += 4;
        }
        else if (t.compare(p, 5, "false") == 0)
        {
            v.kind = Value::Bool;
            p += 5;
        }
        else if (t.compare(p, 4, "null") == 0)
            p += 4;
        else
        {
            char* end = nullptr;
            const char* start = t.c_str() + p;
            // Hex numbers (0x...) are allowed for RAM addresses.
            if (t.compare(p, 2, "0x") == 0)
                v.n = (double)std::strtoull(start, &end, 16);
            else
                v.n = std::strtod(start, &end);
            if (end == start)
                fail("unexpected character");
            v.kind = Value::Number;
            p += end - start;
        }
        return v;
    }
};

}  // namespace

Value parse(const std::string& text)
{
    Parser ps{text};
    Value v = ps.value();
    ps.ws();
    if (ps.p != text.size())
        ps.fail("trailing data");
    return v;
}

}  // namespace json
